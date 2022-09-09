## 下载微博用户图片

### 使用方法

1. 设置微博 cookie 环境变量 `WB_COOKIE`
2. 

```
USAGE:
    download-weibo-image [OPTIONS] <UID>

ARGS:
    <UID>    下载用户数字ID

OPTIONS:
    -c, --concurrency <CONCURRENCY>    下载并发数 [default: 10]
    -e, --end <END>                    结束时间范围
    -h, --help                         Print help information
    -o, --output <OUTPUT>              保存目录, 默认当前目录 [default: .]
    -r, --retry <RETRY>                下载失败重试次数 [default: 3]
    -s, --start <START>                开始时间范围
    -V, --version                      Print version information
```