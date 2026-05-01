// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
#include "DeviceManager.h"
#include "CableInfo.h"

namespace WhatCable {

void DeviceManager::startMonitoring()
{
    m_monitor.start();
    refresh();
}

void DeviceManager::refresh()
{
    m_devices.clear();

    m_usbDevices = UsbDevice::enumerate();
    m_typecPorts = TypeCPort::enumerate();
    m_pdPorts = PowerDeliveryPort::enumerate();

    for (const auto &tcPort : m_typecPorts) {
        std::optional<PowerDeliveryPort> pd;
        for (const auto &pdPort : m_pdPorts) {
            if (pdPort.parentPortNumber == tcPort.portNumber) {
                pd = pdPort;
                break;
            }
        }
        if (!pd && m_pdPorts.size() == 1 && m_typecPorts.size() == 1)
            pd = m_pdPorts[0];

        std::optional<CableInfo> cable;
        if (tcPort.cable)
            cable = CableInfo::fromTypeCCable(*tcPort.cable);

        m_devices.push_back(DeviceSummary::fromTypeCPort(tcPort, pd, cable));
    }

    for (const auto &dev : m_usbDevices) {
        if (dev.isRootHub)
            continue;
        m_devices.push_back(DeviceSummary::fromUsbDevice(dev));
    }

    if (m_devicesChanged)
        m_devicesChanged();
}

} // namespace WhatCable
