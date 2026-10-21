# lanx-api

服务内网请求的模块化 Rust API，使用 Rust 1.99.1 / Edition 2024、Axum、Tokio 和 Reqwest。Docker 目标平台为 Linux ARM64（aarch64）。

## 本地运行

```sh
cargo run
# 指定真实上游地址：
SMS_URL=http://192.168.1.20:9000/events cargo run
```

环境变量：

| 变量 | 默认值 | 用途 |
| --- | --- | --- |
| `SMS_URL` | `http://127.0.0.1:9000/events` | 固定上游 POST 地址，启动时校验 HTTP(S) URL |
| `SMS_CONNECT_TIMEOUT_SECS` | `3` | 上游连接超时（正整数秒） |
| `SMS_REQUEST_TIMEOUT_SECS` | `10` | 上游请求总超时（正整数秒） |
| `RUST_LOG` | `info` | JSON 日志过滤级别 |
| `PORT` | `8383` | Compose 宿主对外端口 |
| `BIND_IP` | `127.0.0.1` | Compose 宿主端口绑定地址；内网访问时设置为宿主的内网 IP |

配置在启动时读取并校验一次；仅变量缺失时采用默认值，空值、格式错误、非 Unicode 值或非正数超时会导致启动失败，并指出变量名。程序固定监听 `0.0.0.0:8383`，不再读取 `LISTEN_ADDR` 或 `SERVER_LISTEN_ADDR`。

Rust 配置按模块分别维护，部署仍使用一份 `.env`。Compose 通过 `BIND_IP` 和 `PORT` 控制宿主绑定地址及对外端口，容器内部端口固定为 `8383`。修改对外端口只需修改 `.env`，然后执行 `docker compose up -d` 重新创建容器，无需重新构建镜像。下方 curl 使用默认端口 `8383`。

本地 `cargo run` 不会自动读取 `.env`；可直接设置 shell 环境变量。Compose 自动读取 `.env`。

## 接口

### GET /

返回 HTTP 307 临时重定向到 `/health`，支持跟随重定向的健康检查。

### GET /health

```sh
curl http://localhost:8383/health
# {"status":"ok"}
```

仅表示服务存活，不检查上游是否可用。

### POST /api/v1/sms

```sh
curl -X POST http://localhost:8383/api/v1/sms \
  -H 'Content-Type: application/json' \
  -d '{"message":"验证码：123456","timestamp":"2026-10-20T12:00:00+08:00","sender":"10690000"}'
```

SMS 模块在字段解析前以 `info` 级别记录请求方法、URI、请求头和完整原始 Body，额外字段和无效 JSON 也会记录。保留 1 MiB 请求体上限，超限请求记录读取失败日志并返回 413。

SMS 请求的 `message`、`timestamp`、`sender` 均为必填字符串，按此顺序用换行符拼接，不裁剪或修改原文。向 `SMS_URL` 发送：

```json
{"msg_type":"text","content":{"text":"验证码：123456\n2026-10-20T12:00:00+08:00\n10690000"}}
```

旧 `/api/v1/forward` 路由已替换为 `/api/v1/sms`，环境变量前缀由 `FORWARD_` 改为 `SMS_`，请同步修改调用方和部署配置。

收到上游任意 2xx 后返回 HTTP 200：

```json
{"status":"forwarded","upstream_status":200}
```

格式错误的 JSON 返回 400，字段缺失或类型错误返回 422，非 JSON Content-Type 返回 415，请求体超过 1 MiB 返回 413。上游非 2xx 或连接失败返回 502，超时返回 504。上游响应正文不回传、不加载；请求总超时 10 秒，连接超时 3 秒，不跟随重定向，不自动重试。客户端重试可能重复投递，需业务端自行定义幂等规则。

默认上游是本地占位地址，尚未配置实际服务时转发会返回 502。容器内 `127.0.0.1` 指向容器自身；实际部署可改为内网 IP 或同一 Docker 网络中的服务名。

## ARM64 Docker 部署

在 ARM64 主机上：

```sh
cp .env.example .env
# 编辑 .env，设置 SMS_URL、宿主内网 BIND_IP 和对外 PORT
docker compose up -d --build
```

在其他架构机器上构建 ARM64 镜像（需要 buildx 和 ARM64 模拟支持）：

```sh
docker buildx build --platform linux/arm64 -t lanx-api:latest --load .
docker save lanx-api:latest -o lanx-api-arm64.tar
# 将 tar 复制到 ARM64 主机，然后 docker load -i lanx-api-arm64.tar
```

容器使用非 root 用户运行，支持 SIGTERM / Ctrl-C 优雅退出。没有内置认证，部署时应通过宿主端口绑定和内网防火墙限制访问范围。

## 模块组织与开发

- `src/main.rs`：启动、日志和退出信号。
- `src/config.rs`：供模块使用的通用环境变量解析工具，不包含服务器配置。
- `src/lib.rs`：调用统一模块入口并应用服务级中间件。
- `src/modules/mod.rs`：集中注册模块，不读取模块配置。
- `src/modules/health.rs`：存活接口。
- `src/modules/sms/mod.rs`：SMS 数据结构、文本转换、客户端创建及转发逻辑。
- `src/modules/sms/config.rs`：`SmsConfig`，管理上游地址和超时。
- `src/modules/sms/tests.rs`：模块测试，使用真实本地 mock 上游。

新增不相关业务时，在 `src/modules/` 中添加独立模块，仅在 `src/modules/mod.rs` 注册。各模块的 `routes()` 自行读取和校验配置、创建依赖、管理状态及路由前缀；`main` 和 `app` 无需了解具体模块。模块配置和测试构造入口均保持私有；修改转发字段时集中调整 `SmsRequest`、`UpstreamRequest` 及其 `From` 转换。

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```
