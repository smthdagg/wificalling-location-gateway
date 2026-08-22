include $(TOPDIR)/rules.mk

PKG_NAME:=wloc-service
PKG_VERSION:=2.0.0
PKG_RELEASE:=1

PKG_MAINTAINER:=wificalling-location-gateway maintainers

include $(INCLUDE_DIR)/package.mk

# The Rust binary is built by scripts/ci/verify-rust-openwrt.sh against the
# pinned OpenWrt 24.10 toolchain and copied into $(BIN_DIR) before packaging.
# This Makefile only installs the binary, init script, and UCI config.

define Package/wloc-service
  SECTION:=net
  CATEGORY:=Network
	TITLE:=WiFi Calling Gateway and WLOC unified service
  DEPENDS:=+nftables +kmod-nft-tproxy +kmod-nft-socket +ip-full
endef

define Package/wloc-ctl
  SECTION:=net
  CATEGORY:=Network
  TITLE:=WLOC control CLI (UDS bridge for LuCI)
  DEPENDS:=+wloc-service
endef

define Package/wloc-service/description
  Router-side WiFi Calling Gateway and WLOC control daemon. Serves the frozen
  wloc.service/v1 control API over a root-only Unix socket and shares the
  integrated supervisor with the Gateway data plane.
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
	$(INSTALL_DATA) ./files/etc/config/wificalling-gateway $(1)/etc/config/
	$(INSTALL_BIN) ./files/etc/init.d/wificalling-gateway $(1)/etc/init.d/
	$(INSTALL_DIR) $(1)/usr/libexec/wificalling-gateway
	$(INSTALL_BIN) ./files/usr/libexec/wificalling-gateway/*.sh $(1)/usr/libexec/wificalling-gateway/
	$(INSTALL_DIR) $(1)/usr/share/rpcd/acl.d
	$(INSTALL_DATA) ./files/usr/share/rpcd/acl.d/luci-app-wificalling-gateway.json $(1)/usr/share/rpcd/acl.d/
	$(INSTALL_DIR) $(1)/www/luci-static/resources/view/wificalling-gateway
	$(INSTALL_DATA) ./files/www/luci-static/resources/view/wificalling-gateway/*.js $(1)/www/luci-static/resources/view/wificalling-gateway/
	$(INSTALL_DIR) $(1)/www/luci-static/resources/wificalling-gateway
	$(INSTALL_DATA) ./files/www/luci-static/resources/wificalling-gateway/*.js $(1)/www/luci-static/resources/wificalling-gateway/
endef

define Package/wloc-ctl/install
	$(INSTALL_DIR) $(1)/usr/sbin
	$(INSTALL_BIN) $(BIN_DIR)/wloc-ctl $(1)/usr/sbin/
endef

$(eval $(call BuildPackage,wloc-service))
$(eval $(call BuildPackage,wloc-ctl))
