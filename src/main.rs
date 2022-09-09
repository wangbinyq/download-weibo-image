use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use async_channel::Receiver;
use chrono::{Datelike, Local};
use clap::Parser;
use futures_util::{future::join_all, TryFutureExt};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{header::HeaderMap, Client};
use tokio::io::AsyncWriteExt;

mod types;

use types::*;

async fn fetch_user_image_wall(
    client: &Client,
    uid: i64,
    sinceid: &str,
    retry: u32,
) -> Result<ImageWallRes> {
    for _ in 0..retry {
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
            .and_then(|res| res.json())
            .await;

        if res.is_ok() {
            return res.context("");
        }
    }

    bail!("获取用户图片列表失败")
}

async fn download_image_task(
    n: u32,
    args: Args,
    client: Client,
    pb: Arc<ProgressBar>,
    r: Receiver<ImageItem>,
) {
    while let Ok(img) = r.recv().await {
        tokio::fs::create_dir_all(&img.mid).await.ok();
        let output = format!("{}/{}.jpg", img.mid, img.pid);
        let mut output = tokio::fs::File::create(output).await.unwrap();

        'retry: for retry in 0..args.retry {
            if retry > 0 {
                pb.println(format!("Task {n} Download {} retry {retry}", img.pid));
            }

            if let Ok(mut res) = client
                .get(format!(
                    "https://wx1.sinaimg.cn/{}/{}.jpg",
                    args.image_type, img.pid
                ))
                .send()
                .await
            {
                loop {
                    match res.chunk().await {
                        Ok(Some(chunk)) => {
                            output.write_all(&chunk).await.unwrap();
                        }
                        Ok(None) => {
                            break 'retry;
                        }
                        Err(_) => {
                            continue 'retry;
                        }
                    }
                }
            }
        }

        pb.inc(1);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let args = Args::parse();

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
    let mut year = Local::now().year() as u32;
    let mut month = Local::now().month();

    let (send, recv) = async_channel::unbounded();

    let pb = Arc::new(
        ProgressBar::new(100).with_style(
            ProgressStyle::with_template(
                "{spinner} [{elapsed_precise}] {bar:40.cyan/blue} ({human_pos}/{human_len}) {eta_precise}",
            )
            .unwrap()
            .progress_chars("##-"),
        ),
    );

    let tasks: Vec<_> = (0..args.concurrency)
        .map(|n| {
            tokio::spawn(download_image_task(
                n + 1,
                args.clone(),
                client.clone(),
                Arc::clone(&pb),
                recv.clone(),
            ))
        })
        .collect();
    'get_images: loop {
        let ImageWallRes {
            ok,
            data,
            bottom_tips_text,
            bottom_tips_visible,
        } = fetch_user_image_wall(&client, args.uid, &sinceid, args.retry).await?;

        if ok != 1 {
            bail!("获取用户图片列表失败");
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

            pb.inc_length(1);
            send.send(img).await.unwrap();
        }

        sinceid = data.since_id;
        if sinceid.is_empty() || sinceid == "0" {
            break;
        }
    }

    drop(send);

    join_all(tasks).await;

    println!(
        "下载完成, 总共 {} 张图片, 耗时: {:?}",
        pb.length().unwrap(),
        pb.elapsed()
    );

    Ok(())
}
