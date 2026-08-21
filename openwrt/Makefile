include $(TOPDIR)/rules.mk

PKG_NAME:=wloc-service
PKG_VERSION:=1.2.0
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
  DEPENDS:=+nftables +kmod-nft-tproxy +kmod-nft-socket +ip-full
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
	$(INSTALL_BIN) ./files/usr/sbin/wloc-redirect-sync.sh $(1)/usr/sbin/
	$(INSTALL_BIN) ./files/usr/sbin/wloc-refresh-set.sh $(1)/usr/sbin/
	$(INSTALL_BIN) ./files/usr/sbin/wloc-profile-redirect.sh $(1)/usr/sbin/
	$(INSTALL_BIN) ./files/usr/sbin/wloc-profile-status.sh $(1)/usr/sbin/
	$(INSTALL_BIN) ./files/usr/sbin/wloc-health.sh $(1)/usr/sbin/
	$(INSTALL_BIN) ./files/usr/sbin/wloc-support-bundle.sh $(1)/usr/sbin/
	$(INSTALL_BIN) ./files/usr/sbin/wloc-component-update.sh $(1)/usr/sbin/
	$(INSTALL_DIR) $(1)/usr/share/wificalling-location-gateway
	$(INSTALL_DATA) ./files/usr/share/wificalling-location-gateway/resource-budget.conf \
		$(1)/usr/share/wificalling-location-gateway/
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./files/etc/init.d/wloc-service $(1)/etc/init.d/
	$(INSTALL_BIN) ./files/etc/init.d/wificalling-location-gateway $(1)/etc/init.d/
	$(INSTALL_DIR) $(1)/usr/libexec/wificalling-location-gateway
	$(INSTALL_BIN) ./files/usr/libexec/wificalling-location-gateway/unified-supervisor.sh $(1)/usr/libexec/wificalling-location-gateway/
	$(INSTALL_BIN) ./files/usr/libexec/wificalling-location-gateway/singbox-runtime.sh $(1)/usr/libexec/wificalling-location-gateway/
	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_DATA) ./files/etc/config/wloc-service $(1)/etc/config/
endef

define Package/wloc-ctl/install
	$(INSTALL_DIR) $(1)/usr/sbin
	$(INSTALL_BIN) $(BIN_DIR)/wloc-ctl $(1)/usr/sbin/
endef

$(eval $(call BuildPackage,wloc-service))
$(eval $(call BuildPackage,wloc-ctl))
