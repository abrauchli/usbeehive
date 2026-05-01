// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Linux equivalent of USBDevice.swift + USBWatcher.swift
#include "UsbDevice.h"
#include "SysfsReader.h"
#include <algorithm>
#include <cctype>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <unordered_map>

namespace fs = std::filesystem;

namespace WhatCable {

static const char kUsbDevicesPath[] = "/sys/bus/usb/devices";

std::string UsbDevice::displayName() const
{
    if (!product.empty())
        return product;
    char buf[32];
    std::snprintf(buf, sizeof(buf), "%04x:%04x", vendorId, productId);
    return buf;
}

std::string UsbDevice::speedLabel() const
{
    if (speed >= 20000) return "USB4 20 Gbps";
    if (speed >= 10000) return "SuperSpeed+ 10 Gbps";
    if (speed >= 5000)  return "SuperSpeed 5 Gbps";
    if (speed >= 480)   return "High Speed 480 Mbps";
    if (speed >= 12)    return "Full Speed 12 Mbps";
    if (speed >= 2)     return "Low Speed 1.5 Mbps";
    return "Unknown speed";
}

std::string UsbDevice::powerLabel() const
{
    if (maxPowerMA <= 0)
        return {};
    char buf[48];
    if (maxPowerMA >= 1000)
        std::snprintf(buf, sizeof(buf), "%.1f W", maxPowerMA / 1000.0);
    else
        std::snprintf(buf, sizeof(buf), "%d mA", maxPowerMA);
    return buf;
}

static int parseMaxPower(std::string_view val)
{
    int n = 0;
    for (unsigned char c : val) {
        if (std::isdigit(c))
            n = n * 10 + static_cast<int>(c - '0');
    }
    return n;
}

std::optional<UsbDevice> UsbDevice::fromSysfs(const std::string &path, const std::string &name)
{
    if (name.find(':') != std::string::npos)
        return std::nullopt;

    UsbDevice dev;
    dev.sysfsPath = path;
    dev.busPort = name;

    auto vid = SysfsReader::readHexAttribute(path + "/idVendor");
    auto pid = SysfsReader::readHexAttribute(path + "/idProduct");
    if (!vid || !pid)
        return std::nullopt;

    dev.vendorId = static_cast<uint16_t>(*vid);
    dev.productId = static_cast<uint16_t>(*pid);
    dev.manufacturer = SysfsReader::readAttribute(path + "/manufacturer");
    dev.product = SysfsReader::readAttribute(path + "/product");
    dev.serial = SysfsReader::readAttribute(path + "/serial");
    {
        std::string ver = SysfsReader::readAttribute(path + "/version");
        auto trim = [](std::string &s) {
            while (!s.empty() && std::isspace(static_cast<unsigned char>(s.front()))) s.erase(s.begin());
            while (!s.empty() && std::isspace(static_cast<unsigned char>(s.back()))) s.pop_back();
        };
        trim(ver);
        dev.version = std::move(ver);
    }
    dev.removable = SysfsReader::readAttribute(path + "/removable");

    dev.speed = SysfsReader::readIntAttribute(path + "/speed").value_or(0);
    dev.maxPowerMA = parseMaxPower(SysfsReader::readAttribute(path + "/bMaxPower"));
    dev.busNum = SysfsReader::readIntAttribute(path + "/busnum").value_or(0);
    dev.devNum = SysfsReader::readIntAttribute(path + "/devnum").value_or(0);
    dev.rxLanes = SysfsReader::readIntAttribute(path + "/rx_lanes").value_or(0);
    dev.txLanes = SysfsReader::readIntAttribute(path + "/tx_lanes").value_or(0);
    dev.numConfigurations = SysfsReader::readIntAttribute(path + "/bNumConfigurations").value_or(0);

    auto cls = SysfsReader::readHexAttribute(path + "/bDeviceClass");
    dev.deviceClass = cls ? static_cast<uint8_t>(*cls) : 0;
    auto sub = SysfsReader::readHexAttribute(path + "/bDeviceSubClass");
    dev.deviceSubClass = sub ? static_cast<uint8_t>(*sub) : 0;
    auto proto = SysfsReader::readHexAttribute(path + "/bDeviceProtocol");
    dev.deviceProtocol = proto ? static_cast<uint8_t>(*proto) : 0;

    dev.isHub = (dev.deviceClass == 0x09);
    dev.isRootHub = (name.compare(0, 3, "usb") == 0);

    std::string numIf = SysfsReader::readAttribute(path + "/bNumInterfaces");
    dev.numInterfaces = static_cast<int>(std::strtol(numIf.c_str(), nullptr, 10));

    std::error_code ec;
    for (const auto &e : fs::directory_iterator(fs::path(path), fs::directory_options::skip_permission_denied, ec)) {
        if (!ec && e.is_directory(ec)) {
            const std::string entry = e.path().filename().string();
            if (entry.find(':') == std::string::npos)
                continue;
            const std::string ifPath = e.path().string();
            UsbInterface iface;
            auto ifClass = SysfsReader::readHexAttribute(ifPath + "/bInterfaceClass");
            if (!ifClass)
                continue;
            iface.classCode = static_cast<uint8_t>(*ifClass);
            auto ifSub = SysfsReader::readHexAttribute(ifPath + "/bInterfaceSubClass");
            iface.subClass = ifSub ? static_cast<uint8_t>(*ifSub) : 0;
            auto ifProto = SysfsReader::readHexAttribute(ifPath + "/bInterfaceProtocol");
            iface.protocol = ifProto ? static_cast<uint8_t>(*ifProto) : 0;

            fs::path driverLink = fs::path(ifPath) / "driver";
            if (fs::is_symlink(driverLink, ec))
                iface.driver = fs::read_symlink(driverLink, ec).filename().string();

            size_t dot = entry.rfind('.');
            iface.number = dot != std::string::npos && dot + 1 < entry.size()
                ? static_cast<int>(std::strtol(entry.c_str() + dot + 1, nullptr, 10))
                : 0;
            dev.interfaces.push_back(iface);
        }
    }

    dev.rawAttributes = SysfsReader::readAllAttributes(path);

    return dev;
}

void UsbDevice::buildTopology(std::vector<UsbDevice> &devices)
{
    std::unordered_map<std::string, int> nameToIndex;
    nameToIndex.reserve(devices.size());
    for (int i = 0; i < static_cast<int>(devices.size()); ++i)
        nameToIndex[devices[static_cast<size_t>(i)].busPort] = i;

    for (int i = 0; i < static_cast<int>(devices.size()); ++i) {
        const std::string &bp = devices[static_cast<size_t>(i)].busPort;
        if (devices[static_cast<size_t>(i)].isRootHub)
            continue;

        std::string parentBp;
        size_t lastDot = bp.rfind('.');
        if (lastDot != std::string::npos && lastDot > 0)
            parentBp = bp.substr(0, lastDot);
        else {
            size_t dash = bp.find('-');
            if (dash != std::string::npos && dash > 0)
                parentBp = "usb" + bp.substr(0, dash);
        }
        auto it = nameToIndex.find(parentBp);
        if (it != nameToIndex.end())
            devices[static_cast<size_t>(it->second)].children.push_back(devices[static_cast<size_t>(i)]);
    }
}

std::vector<UsbDevice> UsbDevice::enumerate()
{
    std::vector<UsbDevice> devices;
    std::error_code ec;
    const fs::path base(kUsbDevicesPath);
    if (!fs::is_directory(base, ec))
        return devices;

    std::vector<std::string> names;
    for (const auto &e : fs::directory_iterator(base, fs::directory_options::skip_permission_denied, ec)) {
        if (!ec && e.is_directory(ec))
            names.push_back(e.path().filename().string());
    }
    std::sort(names.begin(), names.end());

    for (const auto &entry : names) {
        auto dev = fromSysfs((base / entry).string(), entry);
        if (dev)
            devices.push_back(std::move(*dev));
    }

    buildTopology(devices);
    return devices;
}

} // namespace WhatCable
