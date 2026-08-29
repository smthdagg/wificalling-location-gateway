include $(TOPDIR)/rules.mk

LUCI_TITLE:=Wi-Fi Calling & WLOC Location Gateway
LUCI_DESCRIPTION:=Unified admin UI for the Wi-Fi Calling gateway and the WLOC location spoofing module (two side-by-side modules sharing the sing-box node pool and routing).
LUCI_DEPENDS:=+wloc-service +luci-app-wificalling-gateway +luci-base +rpcd-mod-rpcsys
LUCI_PKGARCH:=all
PKG_VERSION:=1.3.0
PKG_RELEASE:=11

PKG_MAINTAINER:=wificalling-location-gateway maintainers

include $(TOPDIR)/luci.mk

# call BuildPackage - OpenWrt buildroot signature
