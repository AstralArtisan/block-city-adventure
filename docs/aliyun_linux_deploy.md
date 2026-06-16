# 阿里云 Linux 部署 Dedicated Server

适用对象：本仓库当前的 `server` 二进制。

当前服务端已经改成真正的 headless 启动路径：

- 不再加载 UI、窗口、音频和客户端渲染插件
- 不再依赖贴图 / 字体 / BGM 文件才能启动
- 仍然保留联机、物理、权威模拟和 `assets/configs/*.ron` 数值配置加载

## 0. 推荐测试流

如果你要频繁改代码并上传到阿里云测试，不要传整个仓库。

本仓库已经提供最小源码打包脚本：

```powershell
.\deploy\linux\make-source-bundle.ps1
```

它会生成：

```text
deploy-bundle/block-city-adventure-source.zip
```

这个压缩包只包含：

- `Cargo.toml`
- `Cargo.lock`
- `.cargo/config.toml`
- `src/`
- `assets/configs/`
- `deploy/linux/run-server.sh`
- `deploy/linux/block-city-server.service`
- `deploy/linux/server.env.example`
- `docs/aliyun_linux_deploy.md`

也就是说，后续测试时你只需要反复上传这个 zip，而不是上传整个 `target/`、音频、贴图和其他大文件。

## 1. 现在最少要传哪些文件

如果你在本地已经有 **Linux 版** `server` 二进制，推荐上传这些：

```text
server
assets/configs/
deploy/linux/run-server.sh
deploy/linux/block-city-server.service
deploy/linux/server.env.example
```

可选上传：

```text
saves/
```

说明：

- `server`：服务端可执行文件
- `assets/configs/`：必须。服务端仍然从这里读取平衡参数和房间/敌人/技能配置
- `saves/`：可选。只有你想保留教程标记或其他运行时输出时才需要
- `assets/textures/`、`assets/fonts/`、`assets/bgm/`：服务端现在 **不需要**

如果你不想精简，也可以直接上传整个 `assets/` 目录，服务端照样能跑，只是体积更大。

## 2. 推荐的服务器目录结构

推荐放到：

```text
/opt/block-city-adventure/
  server
  assets/
    configs/
  deploy/
    linux/
      run-server.sh
```

## 3. 运行方式

先给执行权限：

```bash
chmod +x /opt/block-city-adventure/server
chmod +x /opt/block-city-adventure/deploy/linux/run-server.sh
```

前台启动：

```bash
cd /opt/block-city-adventure
./deploy/linux/run-server.sh
```

后台启动：

```bash
cd /opt/block-city-adventure
nohup ./deploy/linux/run-server.sh > server.log 2>&1 &
```

默认端口是 UDP `3457`。

## 4. 如果在阿里云上现编译

把仓库上传到 Linux 机器后：

```bash
cargo build --release --bin server
./target/release/server
```

如果二进制名不是 `server`，请显式传参：

```bash
./target/release/server --coop-server --port 3457
```

## 5. systemd 开机自启

先创建运行用户：

```bash
sudo useradd --system --home /opt/block-city-adventure --shell /usr/sbin/nologin blockcity
sudo chown -R blockcity:blockcity /opt/block-city-adventure
```

复制环境文件并按需修改：

```bash
sudo cp /opt/block-city-adventure/deploy/linux/server.env.example /etc/block-city-server.env
```

复制 service：

```bash
sudo cp /opt/block-city-adventure/deploy/linux/block-city-server.service /etc/systemd/system/
```

重载并启动：

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now block-city-server
```

查看状态：

```bash
sudo systemctl status block-city-server
sudo journalctl -u block-city-server -f
```

## 6. 阿里云必须放行的端口

至少放行：

```text
UDP 3457
```

需要同时检查两层：

- 阿里云安全组
- Linux 机器自身防火墙

## 7. 客户端连接方式

客户端使用阿里云服务器的 **公网 IPv4**：

```powershell
cargo run --bin client -- <公网IPv4> --client-id 1
cargo run --bin client -- <公网IPv4> --client-id 2
```

如果你们用发布版客户端，也是同样的参数形式：

```powershell
client.exe <公网IPv4> --client-id 1
client.exe <公网IPv4> --client-id 2
```

## 8. 部署结论

当前仓库已经支持阿里云 Linux 上的 dedicated coop server，且相比之前：

- 服务端部署包可以缩到 `server + assets/configs`
- 不再需要为服务端准备贴图、字体、BGM
- 不再依赖窗口或本地图形界面才能启动
