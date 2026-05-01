// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Aggregates all USB and Type-C data into DeviceSummary list
#pragma once

#include <functional>
#include <vector>
#include "DeviceSummary.h"
#include "UDevMonitor.h"

namespace WhatCable {

class DeviceManager {
public:
    DeviceManager() = default;

    void refresh();
    void startMonitoring();

    const std::vector<DeviceSummary> &devices() const { return m_devices; }
    int deviceCount() const { return static_cast<int>(m_devices.size()); }

    std::vector<UsbDevice> rawUsbDevices() const { return m_usbDevices; }
    std::vector<TypeCPort> rawTypeCPorts() const { return m_typecPorts; }

    UDevMonitor &udevMonitor() { return m_monitor; }
    const UDevMonitor &udevMonitor() const { return m_monitor; }

    /** Called after each successful refresh(). */
    void setDevicesChangedCallback(std::function<void()> cb) { m_devicesChanged = std::move(cb); }

private:
    UDevMonitor m_monitor;
    std::function<void()> m_devicesChanged;
    std::vector<DeviceSummary> m_devices;
    std::vector<UsbDevice> m_usbDevices;
    std::vector<TypeCPort> m_typecPorts;
    std::vector<PowerDeliveryPort> m_pdPorts;
};

} // namespace WhatCable
