// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Linux equivalent of USBDevice.swift — reads /sys/bus/usb/devices/
#pragma once

#include <map>
#include <optional>
#include <string>
#include <vector>
#include <cstdint>

namespace WhatCable {

struct UsbInterface {
    int number = 0;
    uint8_t classCode = 0;
    uint8_t subClass = 0;
    uint8_t protocol = 0;
    std::string driver;
};

struct UsbDevice {
    std::string sysfsPath;
    std::string busPort;

    uint16_t vendorId = 0;
    uint16_t productId = 0;
    std::string manufacturer;
    std::string product;
    std::string serial;

    std::string version;
    int speed = 0;        // Mbps
    int maxPowerMA = 0;

    uint8_t deviceClass = 0;
    uint8_t deviceSubClass = 0;
    uint8_t deviceProtocol = 0;

    int busNum = 0;
    int devNum = 0;
    int rxLanes = 0;
    int txLanes = 0;
    std::string removable;    // "removable", "fixed", "unknown"

    int numInterfaces = 0;
    int numConfigurations = 0;

    std::vector<UsbInterface> interfaces;
    std::vector<UsbDevice> children;

    bool isHub = false;
    bool isRootHub = false;

    std::map<std::string, std::string> rawAttributes;

    std::string displayName() const;
    std::string speedLabel() const;
    std::string powerLabel() const;

    static std::vector<UsbDevice> enumerate();

private:
    static std::optional<UsbDevice> fromSysfs(const std::string &path, const std::string &name);
    static void buildTopology(std::vector<UsbDevice> &devices);
};

} // namespace WhatCable
