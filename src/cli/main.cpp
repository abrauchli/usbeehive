// WhatCable-Linux CLI
// Port of WhatCableCLI from WhatCable by Darryl Morley
// https://github.com/darrylmorley/whatcable
#include "DeviceManager.h"
#include "UsbClassDB.h"
#include "PDDecoder.h"

#include <atomic>
#include <chrono>
#include <cerrno>
#include <csignal>
#include <cstdio>
#include <getopt.h>
#include <iostream>
#include <map>
#include <poll.h>
#include <string>

#ifndef WHATCABLE_LINUX_VERSION
#define WHATCABLE_LINUX_VERSION "0.0.0"
#endif

namespace WhatCable {

namespace {

using Clock = std::chrono::steady_clock;

static const char *RESET = "\033[0m";
static const char *BOLD = "\033[1m";
static const char *DIM = "\033[2m";
static const char *GREEN = "\033[32m";
static const char *YELLOW = "\033[33m";
static const char *BLUE = "\033[34m";
static const char *CYAN = "\033[36m";

std::atomic<bool> g_run{true};

void onSignal(int)
{
    g_run = false;
}

std::string jsonEscape(std::string_view sv)
{
    std::string out;
    out.reserve(sv.size() + 8);
    for (unsigned char uc : sv) {
        char c = static_cast<char>(uc);
        switch (c) {
        case '"': out += "\\\""; break;
        case '\\': out += "\\\\"; break;
        case '\b': out += "\\b"; break;
        case '\f': out += "\\f"; break;
        case '\n': out += "\\n"; break;
        case '\r': out += "\\r"; break;
        case '\t': out += "\\t"; break;
        default:
            if (uc < 0x20) {
                char buf[16];
                std::snprintf(buf, sizeof(buf), "\\u%04x", uc);
                out += buf;
            } else {
                out += c;
            }
        }
    }
    return out;
}

void jsonNlIndent(std::string &s, int depth)
{
    s += '\n';
    for (int i = 0; i < depth * 4; ++i)
        s += ' ';
}

std::string jsonQuoted(std::string_view sv)
{
    return '"' + jsonEscape(sv) + '"';
}

std::string hexVidPid(uint16_t v)
{
    char buf[16];
    std::snprintf(buf, sizeof(buf), "0x%04x", v);
    return buf;
}

void printTextSummary(std::ostream &out, const DeviceManager &mgr, bool showRaw)
{
    const auto &devices = mgr.devices();
    if (devices.empty()) {
        out << "No USB devices found.\n";
        return;
    }

    for (const auto &dev : devices) {
        out << BOLD;
        if (dev.category == DeviceSummary::TypeCPortCategory)
            out << CYAN;
        else if (dev.category == DeviceSummary::HubCategory)
            out << BLUE;
        else
            out << GREEN;

        out << dev.headline << RESET << '\n';

        if (!dev.subtitle.empty())
            out << "  " << dev.subtitle << '\n';

        for (const auto &bullet : dev.bullets)
            out << "  " << DIM << "• " << RESET << bullet << '\n';

        if (dev.chargingDiag) {
            const auto &diag = *dev.chargingDiag;
            if (diag.isWarning)
                out << "  " << YELLOW << "⚠ " << diag.summary << RESET << '\n';
            else
                out << "  " << GREEN << "✓ " << diag.summary << RESET << '\n';
            if (!diag.detail.empty())
                out << "    " << DIM << diag.detail << RESET << '\n';
        }

        if (dev.powerDelivery && !dev.powerDelivery->sourceCapabilities.empty()) {
            out << "  " << BOLD << "Charger profiles:" << RESET << '\n';
            for (const auto &pdo : dev.powerDelivery->sourceCapabilities) {
                std::string marker = pdo.isActive ? std::string(" ◀ active") : std::string();
                out << "    " << pdo.voltageLabel() << " @ " << pdo.currentLabel()
                    << " — " << pdo.powerLabel();
                if (!marker.empty())
                    out << GREEN << marker << RESET;
                out << '\n';
            }
        }

        if (showRaw) {
            const std::map<std::string, std::string> *attrs = nullptr;
            if (dev.usbDevice)
                attrs = &dev.usbDevice->rawAttributes;
            else if (dev.typecPort)
                attrs = &dev.typecPort->rawAttributes;
            if (attrs && !attrs->empty()) {
                out << "  " << DIM << "Raw sysfs attributes:" << RESET << '\n';
                for (const auto &kv : *attrs)
                    out << "    " << kv.first << " = " << kv.second << '\n';
            }
        }

        out << '\n';
    }
}

std::string deviceToJson(const DeviceSummary &dev, bool showRaw, int depth)
{
    std::string s;
    const int d = depth;
    const int d1 = depth + 1;
    const int d2 = depth + 2;
    const int d3 = depth + 3;
    const int d4 = depth + 4;

    jsonNlIndent(s, d);
    s += '{';

    bool needComma = false;
    auto emit = [&](auto &&fn) {
        if (needComma)
            s += ',';
        needComma = true;
        fn();
    };

    emit([&] {
        jsonNlIndent(s, d1);
        s += "\"category\": ";
        if (dev.category == DeviceSummary::TypeCPortCategory)
            s += "\"typec\"";
        else if (dev.category == DeviceSummary::HubCategory)
            s += "\"hub\"";
        else
            s += "\"usb\"";
    });
    emit([&] {
        jsonNlIndent(s, d1);
        s += "\"headline\": " + jsonQuoted(dev.headline);
    });
    emit([&] {
        jsonNlIndent(s, d1);
        s += "\"subtitle\": " + jsonQuoted(dev.subtitle);
    });
    emit([&] {
        jsonNlIndent(s, d1);
        s += "\"icon\": " + jsonQuoted(dev.icon);
    });
    emit([&] {
        jsonNlIndent(s, d1);
        s += "\"bullets\": [";
        for (size_t i = 0; i < dev.bullets.size(); ++i) {
            if (i)
                s += ',';
            jsonNlIndent(s, d2);
            s += jsonQuoted(dev.bullets[i]);
        }
        if (!dev.bullets.empty())
            jsonNlIndent(s, d1);
        s += ']';
    });

    if (dev.usbDevice) {
        const UsbDevice &u = *dev.usbDevice;
        emit([&] {
            jsonNlIndent(s, d1);
            s += "\"usb\": {";
            bool innerComma = false;
            auto emitU = [&](auto &&fn) {
                if (innerComma)
                    s += ',';
                innerComma = true;
                fn();
            };
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"vendorId\": " + jsonQuoted(hexVidPid(u.vendorId));
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"productId\": " + jsonQuoted(hexVidPid(u.productId));
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"manufacturer\": " + jsonQuoted(u.manufacturer);
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"product\": " + jsonQuoted(u.product);
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"speed\": " + std::to_string(u.speed);
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"speedLabel\": " + jsonQuoted(u.speedLabel());
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"version\": " + jsonQuoted(u.version);
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"maxPowerMA\": " + std::to_string(u.maxPowerMA);
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"serial\": " + jsonQuoted(u.serial);
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"removable\": " + jsonQuoted(u.removable);
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"bus\": " + std::to_string(u.busNum);
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"device\": " + std::to_string(u.devNum);
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += std::string("\"isHub\": ") + (u.isHub ? "true" : "false");
            });
            emitU([&] {
                jsonNlIndent(s, d2);
                s += "\"interfaces\": [";
                for (size_t i = 0; i < u.interfaces.size(); ++i) {
                    if (i)
                        s += ',';
                    const auto &iface = u.interfaces[i];
                    jsonNlIndent(s, d3);
                    s += '{';
                    jsonNlIndent(s, d4);
                    s += "\"class\": " + jsonQuoted(UsbClassDB::className(iface.classCode));
                    s += ',';
                    jsonNlIndent(s, d4);
                    s += "\"driver\": " + jsonQuoted(iface.driver);
                    jsonNlIndent(s, d3);
                    s += '}';
                }
                if (!u.interfaces.empty())
                    jsonNlIndent(s, d2);
                s += ']';
            });
            if (showRaw) {
                emitU([&] {
                    jsonNlIndent(s, d2);
                    s += "\"raw\": {";
                    bool firstRaw = true;
                    for (const auto &kv : u.rawAttributes) {
                        if (!firstRaw)
                            s += ',';
                        firstRaw = false;
                        jsonNlIndent(s, d3);
                        s += jsonQuoted(kv.first) + ": " + jsonQuoted(kv.second);
                    }
                    jsonNlIndent(s, d2);
                    s += '}';
                });
            }
            jsonNlIndent(s, d1);
            s += '}';
        });
    }

    if (dev.typecPort) {
        const TypeCPort &tc = *dev.typecPort;
        emit([&] {
            jsonNlIndent(s, d1);
            s += "\"typec\": {";
            bool tcComma = false;
            auto emitTc = [&](auto &&fn) {
                if (tcComma)
                    s += ',';
                tcComma = true;
                fn();
            };
            emitTc([&] {
                jsonNlIndent(s, d2);
                s += "\"port\": " + std::to_string(tc.portNumber);
            });
            emitTc([&] {
                jsonNlIndent(s, d2);
                s += "\"dataRole\": " + jsonQuoted(tc.currentDataRole());
            });
            emitTc([&] {
                jsonNlIndent(s, d2);
                s += "\"powerRole\": " + jsonQuoted(tc.currentPowerRole());
            });
            emitTc([&] {
                jsonNlIndent(s, d2);
                s += "\"portType\": " + jsonQuoted(tc.portType);
            });
            emitTc([&] {
                jsonNlIndent(s, d2);
                s += "\"powerOpMode\": " + jsonQuoted(tc.powerOpMode);
            });
            emitTc([&] {
                jsonNlIndent(s, d2);
                s += std::string("\"connected\": ") + (tc.isConnected() ? "true" : "false");
            });
            if (tc.powerSupply) {
                const TypeCPowerSupply &psy = *tc.powerSupply;
                emitTc([&] {
                    jsonNlIndent(s, d2);
                    s += "\"powerSupply\": {";
                    bool psyComma = false;
                    auto emitPsy = [&](auto &&fn) {
                        if (psyComma)
                            s += ',';
                        psyComma = true;
                        fn();
                    };
                    emitPsy([&] {
                        jsonNlIndent(s, d3);
                        s += "\"name\": " + jsonQuoted(psy.name);
                    });
                    emitPsy([&] {
                        jsonNlIndent(s, d3);
                        s += std::string("\"online\": ") + (psy.online ? "true" : "false");
                    });
                    if (psy.voltageNowUV) {
                        emitPsy([&] {
                            jsonNlIndent(s, d3);
                            s += "\"voltageNowUV\": " + std::to_string(*psy.voltageNowUV);
                        });
                    }
                    if (psy.currentNowUA) {
                        emitPsy([&] {
                            jsonNlIndent(s, d3);
                            s += "\"currentNowUA\": " + std::to_string(*psy.currentNowUA);
                        });
                    }
                    if (psy.currentMaxUA) {
                        emitPsy([&] {
                            jsonNlIndent(s, d3);
                            s += "\"currentMaxUA\": " + std::to_string(*psy.currentMaxUA);
                        });
                    }
                    if (psy.voltageMinUV) {
                        emitPsy([&] {
                            jsonNlIndent(s, d3);
                            s += "\"voltageMinUV\": " + std::to_string(*psy.voltageMinUV);
                        });
                    }
                    if (psy.voltageMaxUV) {
                        emitPsy([&] {
                            jsonNlIndent(s, d3);
                            s += "\"voltageMaxUV\": " + std::to_string(*psy.voltageMaxUV);
                        });
                    }
                    if (psy.voltageNowUV && psy.currentNowUA) {
                        const int64_t mw = static_cast<int64_t>(*psy.voltageNowUV) *
                            *psy.currentNowUA / 1000000000LL;
                        emitPsy([&] {
                            jsonNlIndent(s, d3);
                            s += "\"negotiatedPowerMW\": " + std::to_string(mw);
                        });
                    }
                    emitPsy([&] {
                        jsonNlIndent(s, d3);
                        s += "\"chargeType\": " + jsonQuoted(psy.chargeType);
                    });
                    emitPsy([&] {
                        jsonNlIndent(s, d3);
                        s += "\"usbType\": " + jsonQuoted(psy.usbType);
                    });
                    jsonNlIndent(s, d2);
                    s += '}';
                });
            }
            if (showRaw) {
                emitTc([&] {
                    jsonNlIndent(s, d2);
                    s += "\"raw\": {";
                    bool firstRaw = true;
                    for (const auto &kv : tc.rawAttributes) {
                        if (!firstRaw)
                            s += ',';
                        firstRaw = false;
                        jsonNlIndent(s, d3);
                        s += jsonQuoted(kv.first) + ": " + jsonQuoted(kv.second);
                    }
                    jsonNlIndent(s, d2);
                    s += '}';
                });
            }
            jsonNlIndent(s, d1);
            s += '}';
        });
    }

    if (dev.cable) {
        const CableInfo &c = *dev.cable;
        emit([&] {
            jsonNlIndent(s, d1);
            s += "\"cable\": {";
            bool cComma = false;
            auto emitC = [&](auto &&fn) {
                if (cComma)
                    s += ',';
                cComma = true;
                fn();
            };
            emitC([&] {
                jsonNlIndent(s, d2);
                s += "\"type\": " + jsonQuoted(c.cableType);
            });
            if (c.speed) {
                emitC([&] {
                    jsonNlIndent(s, d2);
                    s += "\"speed\": " + jsonQuoted(cableSpeedLabel(*c.speed));
                });
            }
            if (c.currentRating) {
                emitC([&] {
                    jsonNlIndent(s, d2);
                    s += "\"current\": " + jsonQuoted(cableCurrentLabel(*c.currentRating));
                });
            }
            emitC([&] {
                jsonNlIndent(s, d2);
                s += "\"maxWatts\": " + std::to_string(c.maxWatts);
            });
            emitC([&] {
                jsonNlIndent(s, d2);
                s += "\"vendorId\": " + jsonQuoted(hexVidPid(c.vendorId));
            });
            emitC([&] {
                jsonNlIndent(s, d2);
                s += "\"vendorName\": " + jsonQuoted(c.vendorName);
            });
            jsonNlIndent(s, d1);
            s += '}';
        });
    }

    if (dev.powerDelivery) {
        const PowerDeliveryPort &pd = *dev.powerDelivery;
        emit([&] {
            jsonNlIndent(s, d1);
            s += "\"powerDelivery\": {";
            jsonNlIndent(s, d2);
            s += "\"sourceCapabilities\": [";
            for (size_t i = 0; i < pd.sourceCapabilities.size(); ++i) {
                if (i)
                    s += ',';
                const auto &pdo = pd.sourceCapabilities[i];
                jsonNlIndent(s, d3);
                s += '{';
                jsonNlIndent(s, d4);
                s += "\"type\": " + jsonQuoted(pdo.typeLabel());
                s += ',';
                jsonNlIndent(s, d4);
                s += "\"voltageMV\": " + std::to_string(pdo.voltageMV);
                s += ',';
                jsonNlIndent(s, d4);
                s += "\"currentMA\": " + std::to_string(pdo.currentMA);
                s += ',';
                jsonNlIndent(s, d4);
                s += "\"powerMW\": " + std::to_string(pdo.powerMW);
                s += ',';
                jsonNlIndent(s, d4);
                s += std::string("\"active\": ") + (pdo.isActive ? "true" : "false");
                jsonNlIndent(s, d3);
                s += '}';
            }
            if (!pd.sourceCapabilities.empty())
                jsonNlIndent(s, d2);
            s += ']';
            s += ',';
            jsonNlIndent(s, d2);
            s += "\"maxPowerMW\": " + std::to_string(pd.maxSourcePowerMW);
            jsonNlIndent(s, d1);
            s += '}';
        });
    }

    if (dev.chargingDiag) {
        const ChargingDiagnostic &diag = *dev.chargingDiag;
        emit([&] {
            jsonNlIndent(s, d1);
            s += "\"charging\": {";
            jsonNlIndent(s, d2);
            s += "\"summary\": " + jsonQuoted(diag.summary);
            s += ',';
            jsonNlIndent(s, d2);
            s += "\"detail\": " + jsonQuoted(diag.detail);
            s += ',';
            jsonNlIndent(s, d2);
            s += std::string("\"isWarning\": ") + (diag.isWarning ? "true" : "false");
            jsonNlIndent(s, d1);
            s += '}';
        });
    }

    jsonNlIndent(s, d);
    s += '}';
    return s;
}

void printJsonSummary(std::ostream &out, const DeviceManager &mgr, bool showRaw)
{
    std::string doc = "[";
    const auto &devices = mgr.devices();
    for (size_t i = 0; i < devices.size(); ++i) {
        if (i)
            doc += ',';
        doc += deviceToJson(devices[i], showRaw, 1);
    }
    doc += '\n';
    doc += "]\n";
    out << doc;
}

void watchLoop(DeviceManager &mgr, bool useJson, bool showRaw)
{
    std::signal(SIGINT, onSignal);
    std::signal(SIGTERM, onSignal);

    auto render = [&]() {
        if (!useJson) {
            std::cout << "\033[2J\033[H";
            printTextSummary(std::cout, mgr, showRaw);
        } else {
            printJsonSummary(std::cout, mgr, showRaw);
        }
        std::cout.flush();
    };

    mgr.setDevicesChangedCallback(render);
    mgr.startMonitoring();

    bool dirty = false;
    Clock::time_point deadline = Clock::now();

    pollfd pfd{};

    while (g_run.load()) {
        int timeout_ms = -1;
        auto now = Clock::now();
        if (dirty) {
            auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(deadline - now).count();
            timeout_ms = ms <= 0 ? 0 : static_cast<int>(ms);
            if (timeout_ms < 0)
                timeout_ms = 0;
        }

        pfd.fd = mgr.udevMonitor().fd();
        pfd.events = POLLIN;
        pfd.revents = 0;

        int pr = poll(&pfd, 1, timeout_ms);
        if (pr < 0) {
            if (errno == EINTR && !g_run.load())
                break;
            continue;
        }

        now = Clock::now();

        if ((pfd.revents & POLLIN) != 0) {
            mgr.udevMonitor().drainReceiveQueue();
            dirty = true;
            deadline = now + std::chrono::milliseconds(500);
        }

        now = Clock::now();
        if (dirty && now >= deadline) {
            mgr.refresh();
            dirty = false;
        }
    }

    mgr.setDevicesChangedCallback(nullptr);
    mgr.udevMonitor().stop();
}

void usage(FILE *fp, const char *argv0)
{
    std::fprintf(fp,
                 "Usage: %s [options]\n"
                 "WhatCable-Linux — shows what each USB cable/device can do.\n"
                 "Port of WhatCable (macOS) by Darryl Morley.\n"
                 "\n"
                 "Options:\n"
                 "  --help       Show this help\n"
                 "  --version    Show version\n"
                 "  --json       Structured JSON output\n"
                 "  --watch      Stream updates as devices change\n"
                 "  --raw        Include raw sysfs attributes\n",
                 argv0);
}

} // namespace

} // namespace WhatCable

int main(int argc, char *argv[])
{
    using namespace WhatCable;

    bool useJson = false;
    bool watchMode = false;
    bool showRaw = false;

    static struct option long_opts[] = {
        {"help", no_argument, nullptr, 'h'},
        {"version", no_argument, nullptr, 'V'},
        {"json", no_argument, nullptr, 'j'},
        {"watch", no_argument, nullptr, 'w'},
        {"raw", no_argument, nullptr, 'r'},
        {nullptr, 0, nullptr, 0},
    };

    int c;
    while ((c = getopt_long(argc, argv, "hVjwr", long_opts, nullptr)) != -1) {
        switch (c) {
        case 'h':
            usage(stdout, argc > 0 ? argv[0] : "whatcable-linux");
            return 0;
        case 'V':
            std::printf("whatcable-linux %s\n", WHATCABLE_LINUX_VERSION);
            return 0;
        case 'j':
            useJson = true;
            break;
        case 'w':
            watchMode = true;
            break;
        case 'r':
            showRaw = true;
            break;
        default:
            usage(stderr, argc > 0 ? argv[0] : "whatcable-linux");
            return 2;
        }
    }

    DeviceManager mgr;

    if (watchMode) {
        watchLoop(mgr, useJson, showRaw);
        return 0;
    }

    mgr.refresh();
    if (useJson)
        printJsonSummary(std::cout, mgr, showRaw);
    else
        printTextSummary(std::cout, mgr, showRaw);

    return 0;
}
