include $(TOPDIR)/rules.mk

PKG_NAME:=wloc-service
PKG_VERSION:=1.3.0
PKG_RELEASE:=3

PKG_MAINTAINER:=wificalling-location-gateway maintainers

include $(INCLUDE_DIR)/package.mk

# The Rust binary is built by scripts/ci/verify-rust-openwrt.sh against the
# pinned OpenWrt 24.10 toolchain and copied into $(BIN_DIR) before packaging.
# This Makefile only installs the binary, init script, and UCI config.

define Package/wloc-service
  SECTION:=net
  CATEGORY:=Network
  TITLE:=Wi-Fi Calling WLOC location gateway control daemon
  DEPENDS:=+sing-box
endef

define Package/wloc-ctl
  SECTION:=net
  CATEGORY:=Network
  TITLE:=WLOC control CLI (UDS bridge for LuCI)
  DEPENDS:=+wloc-service
endef

define Package/wloc-service/description
  Router-side WLOC location service control daemon. Serves the frozen
  wloc.service/v1 control API over a root-only Unix socket. Reads
  /etc/config/wloc-service (UCI) at startup for the enabled state,
  location mode, manual coordinates, and presets.
endef

define Package/wloc-ctl/description
  Control CLI bridging LuCI to the root-only wloc-service Unix socket.
  Each invocation sends one framed control request and prints the JSON
  response; the daemon never opens a TCP listener.
endef

define Build/Compile
endef

define Package/wloc-service/install
	$(INSTALL_DIR) $(1)/usr/sbin
	$(INSTALL_BIN) $(BIN_DIR)/wloc-service $(1)/usr/sbin/
	$(INSTALL_BIN) ./files/usr/sbin/export-mobileconfig.sh $(1)/usr/sbin/
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./files/etc/init.d/wloc-service $(1)/etc/init.d/
	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_DATA) ./files/etc/config/wloc-service $(1)/etc/config/
endef

define Package/wloc-ctl/install
	$(INSTALL_DIR) $(1)/usr/sbin
	$(INSTALL_BIN) $(BIN_DIR)/wloc-ctl $(1)/usr/sbin/
endef

$(eval $(call BuildPackage,wloc-service))
$(eval $(call BuildPackage,wloc-ctl))
