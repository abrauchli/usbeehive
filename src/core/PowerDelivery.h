// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Linux equivalent of PowerSource.swift — reads /sys/class/usb_power_delivery/
#pragma once

#include <map>
#include <optional>
#include <string>
#include <vector>
#include <cstdint>

namespace WhatCable {

struct PowerDataObject {
    enum Type { FixedSupply, Battery, VariableSupply, PPS, Unknown };

    Type type = Unknown;
    int voltageMV = 0;
    int maxVoltageMV = 0;   // for PPS/variable
    int currentMA = 0;
    int powerMW = 0;
    bool isActive = false;
    int index = 0;

    std::string voltageLabel() const;
    std::string currentLabel() const;
    std::string powerLabel() const;
    std::string typeLabel() const;
};

struct PowerDeliveryPort {
    std::string sysfsPath;
    std::string name;
    std::string parentPortType;
    int parentPortNumber = -1;

    std::vector<PowerDataObject> sourceCapabilities;
    std::vector<PowerDataObject> sinkCapabilities;

    int maxSourcePowerMW = 0;
    std::optional<int> activeSourcePdoIndex;

    std::map<std::string, std::string> rawAttributes;

    static std::vector<PowerDeliveryPort> enumerate();

private:
    static std::optional<PowerDeliveryPort> fromSysfs(const std::string &path, const std::string &name);
    static std::vector<PowerDataObject> parsePDOs(const std::string &capsPath);
};

} // namespace WhatCable
