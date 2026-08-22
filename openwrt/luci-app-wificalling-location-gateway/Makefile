include $(TOPDIR)/rules.mk

LUCI_TITLE:=WiFi Calling + WLOC Gateway
LUCI_DESCRIPTION:=Unified LuCI administration UI for the in-repository WiFi Calling Gateway and WLOC service.
LUCI_DEPENDS:=+wloc-service +luci-base +rpcd-mod-rpcsys
LUCI_PKGARCH:=all
PKG_VERSION:=2.0.0
PKG_RELEASE:=1

PKG_MAINTAINER:=wificalling-location-gateway maintainers

include $(TOPDIR)/luci.mk

# call BuildPackage - OpenWrt buildroot signature
