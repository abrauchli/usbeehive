// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Linux equivalent of USBCPort.swift — reads /sys/class/typec/
#pragma once

#include <map>
#include <optional>
#include <string>
#include <vector>
#include <cstdint>

namespace WhatCable {

struct TypeCIdentity {
    uint16_t vendorId = 0;
    uint16_t productId = 0;
    std::vector<uint32_t> vdos;
};

struct TypeCPartner {
    std::string type;
    std::optional<TypeCIdentity> identity;
    std::map<std::string, std::string> rawAttributes;
};

struct TypeCCable {
    std::string type;          // "active", "passive"
    std::string plugType;
    std::optional<TypeCIdentity> identity;
    std::map<std::string, std::string> rawAttributes;
};

struct TypeCPort {
    std::string sysfsPath;
    std::string portName;      // "port0", "port1", ...
    int portNumber = -1;

    std::string dataRole;      // "host", "device", "[host]", "[device]"
    std::string powerRole;     // "source", "sink", "[source]", "[sink]"
    std::string portType;      // "dual", "source", "sink"
    std::string powerOpMode;   // "default", "1.5A", "3.0A", "usb_power_delivery"
    std::string orientation;   // "normal", "reverse", "unknown"
    std::string pdRevision;
    std::string usbTypeCRev;

    bool hasPartner = false;
    std::optional<TypeCPartner> partner;
    bool hasCable = false;
    std::optional<TypeCCable> cable;

    std::map<std::string, std::string> rawAttributes;

    bool isConnected() const;
    std::string currentDataRole() const;
    std::string currentPowerRole() const;

    static std::vector<TypeCPort> enumerate();

private:
    static std::optional<TypeCPort> fromSysfs(const std::string &path, const std::string &name);
    static std::optional<TypeCIdentity> readIdentity(const std::string &path);
};

} // namespace WhatCable
