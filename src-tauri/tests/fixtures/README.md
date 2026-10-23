# lsof fixtures

两个 `.lsof` 文件只用于解析器测试，不作为应用数据。它们按本机 macOS lsof 4.91 的实际 `-F0pcftn` 分组格式手工构造，保留 NUL 字段分隔和换行记录边界，不包含用户进程信息。

- `macos-listeners.lsof`：多进程、多 Socket、IPv4/IPv6、重复记录和文件字段顺序变化。
- `malformed-listeners.lsof`：非法端口、缺失名称、无效 PID、未知类型和带 scope 的 IPv6。
