use anyhow::{anyhow, bail, Context, Result};
use chrono::{Datelike, Local};
use clap::Parser;
use reqwest::{header::HeaderMap, Client};
use serde::Deserialize;
use serde_this_or_that::as_string;

#[derive(Debug, Deserialize)]
struct UserInfo {
    screen_name: String,
}

#[derive(Debug, Deserialize)]
struct UserInfoData {
    user: UserInfo,
}

#[derive(Debug, Deserialize)]
struct UserInfoRes {
    data: Option<UserInfoData>,
    ok: i32,
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about=None)]
struct Args {
    /// 下载用户数字ID
    uid: i64,
    /// 保存目录, 默认当前目录
    #[clap(short, long, default_value_t=String::from("."))]
    output: String,
    /// 开始时间范围
    #[clap(short, long)]
    start: Option<String>,
    /// 结束时间范围
    #[clap(short, long)]
    end: Option<String>,
    /// 下载并发数
    #[clap(short, long, default_value_t = 10)]
    concurrency: u32,
    /// 下载失败重试次数
    #[clap(short, long, default_value_t = 3)]
    retry: u32,
}

#[derive(Debug, Deserialize)]
struct ImageItem {
    pid: String,
    mid: String,
    is_paid: bool,
    timeline_month: String,
    timeline_year: String,
    object_id: String,
}

#[derive(Debug, Deserialize)]
struct ImageWallData {
    #[serde(deserialize_with = "as_string")]
    since_id: String,
    list: Vec<ImageItem>,
}

#[derive(Debug, Deserialize)]
struct ImageWallRes {
    data: Option<ImageWallData>,
    bottom_tips_visible: bool,
    bottom_tips_text: String,
    ok: i32,
}

async fn fetch_user_image_wall(client: &Client, uid: i64, sinceid: &str) -> Result<ImageWallRes> {
    let res = client
        .get(format!(
            "https://weibo.com/ajax/profile/getImageWall?uid={}{}",
            uid,
            if sinceid.is_empty() {
                "".to_string()
            } else {
                format!("&sinceid={}", sinceid)
            }
        ))
        .send()
        .await?
        .json()
        .await?;

    Ok(res)
}

async fn fetch_image(item: &ImageItem) -> Result<()> {
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    env_logger::init();

    let mut args = Args::parse();

    let mut start_year_month: Option<(u32, u32)> = None;
    let mut end_year_month: Option<(u32, u32)> = None;

    if let Some(start) = args.start.as_ref() {
        let mut start = start.split("/");
        let err = "开始日期格式需要为 YYYY/MM";
        start_year_month = Some((
            start.next().and_then(|a| a.parse().ok()).context(err)?,
            start.next().and_then(|a| a.parse().ok()).context(err)?,
        ));
    }

    if let Some(end) = args.end.as_ref() {
        let mut end = end.split("/");
        let err = "结束日期格式需要为 YYYY/MM";
        end_year_month = Some((
            end.next().and_then(|a| a.parse().ok()).context(err)?,
            end.next().and_then(|a| a.parse().ok()).context(err)?,
        ));
    }

    let cookie = std::env::var("WB_COOKIE")
        .context("请配置微博COOKIE")
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.append("cookie", cookie.parse().unwrap());

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let err_user_msg = "获取用户信息失败, 请检查 cookie 或者用户 ID 是否正确";

    let user_res: UserInfoRes = client
        .get(format!(
            "https://weibo.com/ajax/profile/info?uid={}",
            args.uid
        ))
        .send()
        .await?
        .json()
        .await
        .context(err_user_msg)?;

    if user_res.ok != 1 {
        println!("error: {:?}", user_res);
        bail!(err_user_msg);
    }

    let user = user_res.data.ok_or(anyhow!("用户信息不存在"))?.user;
    let mut sinceid = "".to_string();
    let mut find = 0;
    let mut download = 0;
    let mut year = Local::now().year() as u32;
    let mut month = Local::now().month();

    println!("开始下载用户[{}]图片", user.screen_name);

    'get_images: loop {
        let ImageWallRes {
            ok,
            data,
            bottom_tips_text,
            bottom_tips_visible,
        } = fetch_user_image_wall(&client, args.uid, &sinceid).await?;

        if ok != 1 {
            println!("获取用户图片列表失败")
        }

        let data = data.unwrap();
        let images = data.list;

        if sinceid.is_empty() && bottom_tips_visible {
            println!("提示: {}", bottom_tips_text);
        }

        for mut img in images {
            if !img.timeline_month.is_empty() {
                if let Ok(m) = img.timeline_month.parse() {
                    month = m;
                } else {
                    println!("解析月份失败: {}", img.timeline_month);
                }
            }

            if !img.timeline_year.is_empty() {
                if let Ok(y) = img.timeline_year.parse() {
                    year = y;
                } else {
                    println!("解析年份失败: {}", img.timeline_year);
                }
            }

            if let Some((start_year, start_month)) = start_year_month {
                if year < start_year {
                    break 'get_images;
                }
                if year == start_year && month < start_month {
                    break 'get_images;
                }
            }

            if let Some((end_year, end_month)) = end_year_month {
                if year > end_year {
                    continue;
                }
                if year == end_year && month > end_month {
                    continue;
                }
            }

            img.mid = format!("{}/{}/{}-{}", args.output, user.screen_name, year, month);

            tokio::fs::create_dir_all(&img.mid).await?;

            let body = client
                .get(format!("https://wx1.sinaimg.cn/large/{}.jpg", img.pid))
                .send()
                .await?
                .bytes()
                .await?;

            tokio::fs::write(format!("{}/{}.jpg", img.mid, img.pid), body).await?;

            find += 1;
        }

        println!("下载图片{}张", find);
        sinceid = data.since_id;
        if sinceid.is_empty() {
            break;
        }
    }

    Ok(())
}
