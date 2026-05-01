// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// udev-based hotplug monitoring for USB and Type-C subsystems
#pragma once

struct udev;
struct udev_monitor;

namespace WhatCable {

class UDevMonitor {
public:
    UDevMonitor();
    ~UDevMonitor();

    bool start();
    void stop();
    bool isRunning() const { return m_running; }

    /** libudev socket fd for poll(); -1 when not running */
    int fd() const { return m_running ? m_fd : -1; }

    /** Drain pending kernel messages (call when fd becomes readable). */
    void drainReceiveQueue();

private:
    udev *m_udev = nullptr;
    udev_monitor *m_monitor = nullptr;
    bool m_running = false;
    int m_fd = -1;
};

} // namespace WhatCable
