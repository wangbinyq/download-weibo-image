use clap::Parser;
use serde::Deserialize;
use serde_this_or_that::as_string;

#[derive(Debug, Deserialize)]
pub struct UserInfo {
    pub screen_name: String,
}

#[derive(Debug, Deserialize)]
pub struct UserInfoData {
    pub user: UserInfo,
}

#[derive(Debug, Deserialize)]
pub struct UserInfoRes {
    pub data: Option<UserInfoData>,
    pub ok: i32,
}

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about=None)]
pub struct Args {
    /// 下载用户数字ID
    pub uid: i64,
    /// 保存目录, 默认当前目录
    #[clap(short, long, default_value_t=String::from("."))]
    pub output: String,
    /// 开始时间范围
    #[clap(short, long)]
    pub start: Option<String>,
    /// 结束时间范围
    #[clap(short, long)]
    pub end: Option<String>,
    /// 下载并发数
    #[clap(short, long, default_value_t = 10)]
    pub concurrency: u32,
    /// 下载失败重试次数
    #[clap(short, long, default_value_t = 3)]
    pub retry: u32,
    /// 图片大小
    #[clap(short, long, default_value_t = String::from("mw600"))]
    pub image_type: String,
}

#[derive(Debug, Deserialize)]
pub struct ImageItem {
    pub pid: String,
    pub mid: String,
    pub timeline_month: String,
    pub timeline_year: String,
}

#[derive(Debug, Deserialize)]
pub struct ImageWallData {
    #[serde(deserialize_with = "as_string")]
    pub since_id: String,
    pub list: Vec<ImageItem>,
}

#[derive(Debug, Deserialize)]
pub struct ImageWallRes {
    pub data: Option<ImageWallData>,
    pub bottom_tips_visible: bool,
    pub bottom_tips_text: String,
    pub ok: i32,
}
