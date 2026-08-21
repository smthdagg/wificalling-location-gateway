include $(TOPDIR)/rules.mk

LUCI_TITLE:=WLOC Location Service
LUCI_DESCRIPTION:=Standalone LuCI administration UI for the WLOC location service.
LUCI_DEPENDS:=+wloc-service +luci-base +rpcd-mod-rpcsys
LUCI_PKGARCH:=all
PKG_VERSION:=2.0.0
PKG_RELEASE:=1

PKG_MAINTAINER:=wificalling-location-gateway maintainers

include $(TOPDIR)/luci.mk

# call BuildPackage - OpenWrt buildroot signature
