# WLOC 定位服务教程

这是 V2 独立仓库的 WiFi Calling Gateway + WLOC 一体化教程。本项目包含两者，
不安装或依赖外部 Wi-Fi Calling Gateway 1.7 仓库。

1. 安装与路由器架构匹配的 OpenWrt 软件包，保留 sing-box tiny/lite 或
   PassWall 提供的 sing-box。
2. 打开“服务 → WiFi Calling + WLOC Gateway → WCG Setting”配置 Gateway 节点。
3. 打开“WLOC Setting”，启用 WLOC，选择定位提供程序，确认提供程序配置路径。
4. 打开“WLOC Devices”，为每台授权局域网设备建立一个档案，填写私有地址、明确的
   WLOC 节点，并启用档案。
5. 选择“自动跟随选定节点”或“手动定位”。手动经纬度写入同一设备档案。
6. 应用配置，在“WCG WLOC Service Monitor”确认守护进程、提供程序、重定向和设备档案正常。
7. 只在授权测试 iPhone 上安装并信任本地 CA，触发地图或天气请求后查看“WLOC Status & Logs”。
8. “Component Update”是独立页面：先放入签名软件包并预检，再应用；如有中断事务，
   先恢复后再开始下一次更新。

AX6S 存储空间有限，安装前应卸载旧 WLOC 软件包，但不要卸载正在使用的提供程序。
