// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Port of PortSummary.swift — generates human-readable summary per device/port
#pragma once

#include <optional>
#include <string>
#include <vector>
#include "UsbDevice.h"
#include "TypeCPort.h"
#include "PowerDelivery.h"
#include "CableInfo.h"
#include "ChargingDiagnostic.h"

namespace WhatCable {

struct DeviceSummary {
    enum Category { UsbDeviceCategory, TypeCPortCategory, HubCategory };
    enum Status { Empty, Connected, Charging, DataDevice, ThunderboltDevice, Unknown };

    Category category = UsbDeviceCategory;
    Status status = Empty;

    std::string headline;
    std::string subtitle;
    std::vector<std::string> bullets;
    std::string icon;

    std::optional<UsbDevice> usbDevice;

    std::optional<TypeCPort> typecPort;
    std::optional<PowerDeliveryPort> powerDelivery;
    std::optional<CableInfo> cable;
    std::optional<ChargingDiagnostic> chargingDiag;

    static DeviceSummary fromUsbDevice(const UsbDevice &dev);
    static DeviceSummary fromTypeCPort(
        const TypeCPort &port,
        const std::optional<PowerDeliveryPort> &pd,
        const std::optional<CableInfo> &cable);
};

} // namespace WhatCable
