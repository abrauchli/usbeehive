// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Port of PortSummary.swift — plain-English summary logic
#include "DeviceSummary.h"
#include "VendorDB.h"
#include "UsbClassDB.h"
#include "PDDecoder.h"
#include <algorithm>
#include <cstdio>

namespace WhatCable {

namespace {

bool startsWithHexVid(const std::string &vendorName)
{
    return vendorName.size() >= 2 && vendorName[0] == '0' &&
           (vendorName[1] == 'x' || vendorName[1] == 'X');
}

bool vecContains(const std::vector<std::string> &v, const std::string &s)
{
    return std::find(v.begin(), v.end(), s) != v.end();
}

std::string joinComma(const std::vector<std::string> &v)
{
    std::string out;
    for (size_t i = 0; i < v.size(); ++i) {
        if (i)
            out += ", ";
        out += v[i];
    }
    return out;
}

} // namespace

DeviceSummary DeviceSummary::fromUsbDevice(const UsbDevice &dev)
{
    DeviceSummary s;
    s.category = dev.isHub ? HubCategory : UsbDeviceCategory;
    s.status = Connected;
    s.usbDevice = dev;

    std::string vendorName = VendorDB::lookup(dev.vendorId);
    bool hasVendorName = !startsWithHexVid(vendorName);

    s.headline = dev.displayName();

    std::string deviceType;
    if (dev.deviceClass != 0 && dev.deviceClass != 0xFF) {
        deviceType = UsbClassDB::className(dev.deviceClass);
    } else if (!dev.interfaces.empty()) {
        std::vector<std::string> types;
        for (const auto &iface : dev.interfaces) {
            std::string t = UsbClassDB::className(iface.classCode);
            if (!vecContains(types, t) && t != "Composite" && !startsWithHexVid(t))
                types.push_back(t);
        }
        deviceType = joinComma(types);
    }

    if (hasVendorName)
        s.subtitle = vendorName;
    if (!deviceType.empty()) {
        if (!s.subtitle.empty())
            s.subtitle += " · ";
        s.subtitle += deviceType;
    }

    s.bullets.push_back(dev.speedLabel());

    if (dev.maxPowerMA > 0)
        s.bullets.push_back("Power: " + dev.powerLabel());

    s.bullets.push_back("USB " + dev.version);

    if (!dev.serial.empty()) {
        char buf[128];
        std::snprintf(buf, sizeof(buf), "Serial: %s", dev.serial.c_str());
        s.bullets.emplace_back(buf);
    }

    if (dev.removable == "removable")
        s.bullets.emplace_back("Removable");
    else if (dev.removable == "fixed")
        s.bullets.emplace_back("Built-in");

    std::vector<std::string> drivers;
    for (const auto &iface : dev.interfaces) {
        if (!iface.driver.empty() && !vecContains(drivers, iface.driver))
            drivers.push_back(iface.driver);
    }
    if (!drivers.empty())
        s.bullets.push_back("Drivers: " + joinComma(drivers));

    char vidpid[24];
    std::snprintf(vidpid, sizeof(vidpid), "VID:PID %04x:%04x", dev.vendorId, dev.productId);
    s.bullets.emplace_back(vidpid);

    if (dev.isHub)
        s.icon = "network-wired";
    else if (deviceType.find("Audio") != std::string::npos)
        s.icon = "audio-card";
    else if (deviceType.find("HID") != std::string::npos)
        s.icon = "input-keyboard";
    else if (deviceType.find("Mass Storage") != std::string::npos)
        s.icon = "drive-removable-media";
    else if (deviceType.find("Video") != std::string::npos)
        s.icon = "camera-web";
    else if (deviceType.find("Wireless") != std::string::npos)
        s.icon = "network-wireless";
    else if (deviceType.find("Printer") != std::string::npos)
        s.icon = "printer";
    else
        s.icon = "drive-removable-media-usb";

    return s;
}

DeviceSummary DeviceSummary::fromTypeCPort(
    const TypeCPort &port,
    const std::optional<PowerDeliveryPort> &pd,
    const std::optional<CableInfo> &cableOpt)
{
    DeviceSummary s;
    s.category = TypeCPortCategory;
    s.typecPort = port;
    s.powerDelivery = pd;
    s.cable = cableOpt;
    s.icon = "plug";

    if (!port.isConnected()) {
        s.status = Empty;
        char buf[48];
        std::snprintf(buf, sizeof(buf), "USB-C Port %d", port.portNumber);
        s.headline = buf;
        s.subtitle = "Nothing connected";
        return s;
    }

    s.status = Connected;
    {
        char buf[48];
        std::snprintf(buf, sizeof(buf), "USB-C Port %d", port.portNumber);
        s.headline = buf;
    }

    if (port.hasPartner && port.partner) {
        if (port.partner->identity && !port.partner->identity->vdos.empty()) {
            auto hdr = decodeIDHeader(port.partner->identity->vdos[0]);
            std::string productLabel = productTypeLabel(hdr.ufpProductType);
            std::string vendorLabel = VendorDB::lookup(hdr.vendorId);
            bool hasVendor = !startsWithHexVid(vendorLabel);
            s.subtitle = hasVendor ? vendorLabel + " — " + productLabel : productLabel;
        } else {
            s.subtitle = "Device connected";
        }
    }

    std::string dataStr = port.currentDataRole();
    std::string powerStr = port.currentPowerRole();
    if (!dataStr.empty() || !powerStr.empty()) {
        std::string roleInfo;
        if (!dataStr.empty())
            roleInfo = "Data: " + dataStr;
        if (!powerStr.empty()) {
            if (!roleInfo.empty())
                roleInfo += ", ";
            roleInfo += "Power: " + powerStr;
        }
        s.bullets.push_back(roleInfo);
    }

    if (!port.powerOpMode.empty()) {
        char buf[128];
        std::snprintf(buf, sizeof(buf), "Power mode: %s", port.powerOpMode.c_str());
        s.bullets.emplace_back(buf);
    }

    if (!port.pdRevision.empty()) {
        char buf[128];
        std::snprintf(buf, sizeof(buf), "PD revision: %s", port.pdRevision.c_str());
        s.bullets.emplace_back(buf);
    }

    if (!port.orientation.empty() && port.orientation != "unknown") {
        char buf[128];
        std::snprintf(buf, sizeof(buf), "Plug orientation: %s", port.orientation.c_str());
        s.bullets.emplace_back(buf);
    }

    if (cableOpt) {
        const auto &c = *cableOpt;
        if (c.speed) {
            char buf[160];
            std::snprintf(buf, sizeof(buf), "Cable speed: %s", cableSpeedLabel(*c.speed).c_str());
            s.bullets.emplace_back(buf);
        }
        if (c.currentRating) {
            char buf[160];
            std::snprintf(buf, sizeof(buf), "Cable current: %s", cableCurrentLabel(*c.currentRating).c_str());
            s.bullets.emplace_back(buf);
        }
        if (c.maxWatts > 0) {
            char buf[64];
            std::snprintf(buf, sizeof(buf), "Cable max power: %dW", c.maxWatts);
            s.bullets.emplace_back(buf);
        }
        if (c.isActive)
            s.bullets.emplace_back("Active cable");
        else if (c.isPassive)
            s.bullets.emplace_back("Passive cable");
        if (!c.vendorName.empty() && !startsWithHexVid(c.vendorName)) {
            char buf[160];
            std::snprintf(buf, sizeof(buf), "Cable vendor: %s", c.vendorName.c_str());
            s.bullets.emplace_back(buf);
        }
    }

    if (pd && !pd->sourceCapabilities.empty()) {
        int maxW = pd->maxSourcePowerMW / 1000;
        char buf[64];
        std::snprintf(buf, sizeof(buf), "Charger max: %dW", maxW);
        s.bullets.emplace_back(buf);
        s.status = Charging;
    }

    if (pd) {
        auto diag = ChargingDiagnostic::evaluate(*pd, cableOpt);
        if (diag)
            s.chargingDiag = std::move(diag);
    }

    return s;
}

} // namespace WhatCable
