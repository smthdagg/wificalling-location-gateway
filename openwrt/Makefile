include $(TOPDIR)/rules.mk

PKG_NAME:=wloc-service
PKG_VERSION:=0.1.0
PKG_RELEASE:=1

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

define Package/wloc-service/description
  Router-side WLOC location service control daemon. Serves the frozen
  wloc.service/v1 control API over a root-only Unix socket. Interception
  stays disabled until the Phase 0 evidence and license gates close.
endef

define Build/Compile
endef

define Package/wloc-service/install
	$(INSTALL_DIR) $(1)/usr/sbin
	$(INSTALL_BIN) $(BIN_DIR)/wloc-service $(1)/usr/sbin/
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./files/etc/init.d/wloc-service $(1)/etc/init.d/
	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_DATA) ./files/etc/config/wloc-service $(1)/etc/config/
endef

$(eval $(call BuildPackage,wloc-service))
