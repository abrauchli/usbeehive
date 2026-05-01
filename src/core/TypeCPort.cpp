// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Linux equivalent of USBCPort.swift + USBCPortWatcher.swift
#include "TypeCPort.h"
#include "SysfsReader.h"
#include <algorithm>
#include <cstdlib>
#include <filesystem>
#include <regex>

namespace fs = std::filesystem;

namespace WhatCable {

static const char kTypecPath[] = "/sys/class/typec";

bool TypeCPort::isConnected() const
{
    return hasPartner || hasCable;
}

std::string TypeCPort::currentDataRole() const
{
    static const std::regex re(R"(\[([^\]]+)\])");
    std::smatch match;
    if (std::regex_search(dataRole, match, re) && match.size() > 1)
        return match[1].str();
    return dataRole;
}

std::string TypeCPort::currentPowerRole() const
{
    static const std::regex re(R"(\[([^\]]+)\])");
    std::smatch match;
    if (std::regex_search(powerRole, match, re) && match.size() > 1)
        return match[1].str();
    return powerRole;
}

std::optional<TypeCIdentity> TypeCPort::readIdentity(const std::string &path)
{
    const std::string idPath = path + "/identity";
    if (!SysfsReader::pathExists(idPath))
        return std::nullopt;

    TypeCIdentity id;
    auto vid = SysfsReader::readHexAttribute(idPath + "/id_header");
    if (vid)
        id.vendorId = static_cast<uint16_t>(*vid & 0xFFFF);

    auto pid = SysfsReader::readHexAttribute(idPath + "/product");
    if (pid)
        id.productId = static_cast<uint16_t>(*pid & 0xFFFF);

    std::error_code ec;
    std::vector<std::string> names;
    for (const auto &e : fs::directory_iterator(fs::path(idPath), fs::directory_options::skip_permission_denied, ec)) {
        if (!ec && e.is_regular_file(ec))
            names.push_back(e.path().filename().string());
    }
    std::sort(names.begin(), names.end());

    for (const auto &entry : names) {
        if (!entry.starts_with("vdo") &&
            entry != "id_header" &&
            entry != "cert_stat" &&
            entry != "product" &&
            entry != "product_type_vdo1" &&
            entry != "product_type_vdo2" &&
            entry != "product_type_vdo3")
            continue;

        auto val = SysfsReader::readHexAttribute(idPath + "/" + entry);
        if (val)
            id.vdos.push_back(*val);
    }

    if (id.vendorId == 0 && id.vdos.empty())
        return std::nullopt;

    return id;
}

std::optional<TypeCPort> TypeCPort::fromSysfs(const std::string &path, const std::string &name)
{
    if (!name.starts_with("port"))
        return std::nullopt;

    TypeCPort port;
    port.sysfsPath = path;
    port.portName = name;

    static const std::regex numRe(R"(port(\d+))");
    std::smatch match;
    if (std::regex_match(name, match, numRe) && match.size() > 1)
        port.portNumber = static_cast<int>(std::strtol(match[1].str().c_str(), nullptr, 10));

    port.dataRole = SysfsReader::readAttribute(path + "/data_role");
    port.powerRole = SysfsReader::readAttribute(path + "/power_role");
    port.portType = SysfsReader::readAttribute(path + "/port_type");
    port.powerOpMode = SysfsReader::readAttribute(path + "/power_operation_mode");
    port.orientation = SysfsReader::readAttribute(path + "/orientation");
    port.pdRevision = SysfsReader::readAttribute(path + "/usb_power_delivery_revision");
    port.usbTypeCRev = SysfsReader::readAttribute(path + "/usb_typec_revision");

    const std::string partnerPath = path + "-partner";
    if (SysfsReader::pathExists(partnerPath)) {
        port.hasPartner = true;
        TypeCPartner p;
        p.type = SysfsReader::readAttribute(partnerPath + "/type");
        p.identity = readIdentity(partnerPath);
        p.rawAttributes = SysfsReader::readAllAttributes(partnerPath);
        port.partner = std::move(p);
    }

    const std::string cablePath = path + "-cable";
    if (SysfsReader::pathExists(cablePath)) {
        port.hasCable = true;
        TypeCCable c;
        c.type = SysfsReader::readAttribute(cablePath + "/type");
        c.plugType = SysfsReader::readAttribute(cablePath + "/plug_type");
        c.identity = readIdentity(cablePath);
        c.rawAttributes = SysfsReader::readAllAttributes(cablePath);
        port.cable = std::move(c);
    }

    port.rawAttributes = SysfsReader::readAllAttributes(path);

    return port;
}

std::vector<TypeCPort> TypeCPort::enumerate()
{
    std::vector<TypeCPort> ports;
    if (!SysfsReader::pathExists(kTypecPath))
        return ports;

    std::error_code ec;
    const fs::path base(kTypecPath);
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
