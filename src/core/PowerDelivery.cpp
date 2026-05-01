// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Linux equivalent of PowerSource.swift + PowerSourceWatcher.swift
#include "PowerDelivery.h"
#include "SysfsReader.h"
#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <filesystem>

namespace fs = std::filesystem;

namespace WhatCable {

static const char kPdPath[] = "/sys/class/usb_power_delivery";

std::string PowerDataObject::voltageLabel() const
{
    char buf[64];
    if (type == PPS && maxVoltageMV > 0)
        std::snprintf(buf, sizeof(buf), "%.1f-%.1fV",
                      voltageMV / 1000.0, maxVoltageMV / 1000.0);
    else
        std::snprintf(buf, sizeof(buf), "%.1fV", voltageMV / 1000.0);
    return buf;
}

std::string PowerDataObject::currentLabel() const
{
    char buf[32];
    std::snprintf(buf, sizeof(buf), "%.2fA", currentMA / 1000.0);
    return buf;
}

std::string PowerDataObject::powerLabel() const
{
    char buf[32];
    std::snprintf(buf, sizeof(buf), "%.0fW", powerMW / 1000.0);
    return buf;
}

std::string PowerDataObject::typeLabel() const
{
    switch (type) {
    case FixedSupply: return "Fixed";
    case Battery: return "Battery";
    case VariableSupply: return "Variable";
    case PPS: return "PPS";
    default: return "Unknown";
    }
}

std::vector<PowerDataObject> PowerDeliveryPort::parsePDOs(const std::string &capsPath)
{
    std::vector<PowerDataObject> pdos;
    if (!SysfsReader::pathExists(capsPath))
        return pdos;

    std::error_code ec;
    std::vector<std::string> entries;
    for (const auto &e : fs::directory_iterator(fs::path(capsPath), fs::directory_options::skip_permission_denied, ec)) {
        if (!ec && e.is_directory(ec))
            entries.push_back(e.path().filename().string());
    }
    std::sort(entries.begin(), entries.end());

    for (const auto &entry : entries) {
        const std::string pdoPath = capsPath + "/" + entry;
        PowerDataObject pdo;
        size_t colon = entry.find_last_of(':');
        std::string idxPart = colon == std::string::npos ? entry : entry.substr(colon + 1);
        pdo.index = static_cast<int>(std::strtol(idxPart.c_str(), nullptr, 10));

        std::string typeStr = SysfsReader::readAttribute(pdoPath + "/type");
        if (typeStr == "fixed_supply")
            pdo.type = PowerDataObject::FixedSupply;
        else if (typeStr == "battery")
            pdo.type = PowerDataObject::Battery;
        else if (typeStr == "variable_supply")
            pdo.type = PowerDataObject::VariableSupply;
        else if (typeStr.find("pps") != std::string::npos)
            pdo.type = PowerDataObject::PPS;

        auto voltage = SysfsReader::readIntAttribute(pdoPath + "/voltage");
        if (voltage)
            pdo.voltageMV = *voltage;

        auto maxVoltage = SysfsReader::readIntAttribute(pdoPath + "/maximum_voltage");
        if (maxVoltage)
            pdo.maxVoltageMV = *maxVoltage;
        else if (!voltage) {
            auto minVoltage = SysfsReader::readIntAttribute(pdoPath + "/minimum_voltage");
            if (minVoltage)
                pdo.voltageMV = *minVoltage;
        }

        auto current = SysfsReader::readIntAttribute(pdoPath + "/maximum_current");
        if (!current)
            current = SysfsReader::readIntAttribute(pdoPath + "/current");
        if (current)
            pdo.currentMA = *current;

        auto power = SysfsReader::readIntAttribute(pdoPath + "/maximum_power");
        if (power)
            pdo.powerMW = *power;
        else if (pdo.voltageMV > 0 && pdo.currentMA > 0)
            pdo.powerMW = static_cast<int>(static_cast<int64_t>(pdo.voltageMV) * pdo.currentMA / 1000);

        pdos.push_back(pdo);
    }

    return pdos;
}

std::optional<PowerDeliveryPort> PowerDeliveryPort::fromSysfs(const std::string &path, const std::string &name)
{
    PowerDeliveryPort port;
    port.sysfsPath = path;
    port.name = name;

    port.sourceCapabilities = parsePDOs(path + "/source-capabilities");
    port.sinkCapabilities = parsePDOs(path + "/sink-capabilities");

    for (const auto &pdo : port.sourceCapabilities) {
        if (pdo.powerMW > port.maxSourcePowerMW)
            port.maxSourcePowerMW = pdo.powerMW;
    }

    port.rawAttributes = SysfsReader::readAllAttributes(path);

    if (port.sourceCapabilities.empty() && port.sinkCapabilities.empty())
        return std::nullopt;

    return port;
}

std::vector<PowerDeliveryPort> PowerDeliveryPort::enumerate()
{
    std::vector<PowerDeliveryPort> ports;
    if (!SysfsReader::pathExists(kPdPath))
        return ports;

    std::error_code ec;
    const fs::path base(kPdPath);
    std::vector<std::string> entries;
    for (const auto &e : fs::directory_iterator(base, fs::directory_options::skip_permission_denied, ec)) {
        if (!ec && e.is_directory(ec))
            entries.push_back(e.path().filename().string());
    }
    std::sort(entries.begin(), entries.end());

    for (const auto &entry : entries) {
        auto port = fromSysfs((base / entry).string(), entry);
        if (port)
            ports.push_back(std::move(*port));
    }

    return ports;
}

} // namespace WhatCable
