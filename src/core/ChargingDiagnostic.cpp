// Derived from WhatCable by Darryl Morley (https://github.com/darrylmorley/whatcable)
// Port of ChargingDiagnostic.swift
#include "ChargingDiagnostic.h"
#include <cstdio>

namespace WhatCable {

std::optional<ChargingDiagnostic> ChargingDiagnostic::evaluate(
    const PowerDeliveryPort &pdPort,
    const std::optional<CableInfo> &cable)
{
    if (pdPort.sourceCapabilities.empty())
        return std::nullopt;

    int chargerMaxW = pdPort.maxSourcePowerMW / 1000;
    if (chargerMaxW <= 0)
        return std::nullopt;

    ChargingDiagnostic diag;

    int activeW = 0;
    for (const auto &pdo : pdPort.sourceCapabilities) {
        if (pdo.isActive) {
            activeW = pdo.powerMW / 1000;
            break;
        }
    }
    if (activeW <= 0 && !pdPort.sourceCapabilities.empty())
        activeW = chargerMaxW;

    int cableMaxW = 0;
    if (cable && cable->maxWatts > 0)
        cableMaxW = cable->maxWatts;

    char buf[256];
    if (cableMaxW > 0 && cableMaxW < chargerMaxW) {
        diag.bottleneck = CableLimit;
        diag.summary = "Cable is limiting charging speed";
        std::snprintf(buf, sizeof(buf),
                      "Cable rated for %dW, but charger can deliver %dW",
                      cableMaxW, chargerMaxW);
        diag.detail = buf;
        diag.isWarning = true;
    } else if (activeW > 0 && activeW < chargerMaxW * 0.8) {
        diag.bottleneck = DeviceLimit;
        std::snprintf(buf, sizeof(buf), "Charging at %dW", activeW);
        diag.summary = buf;
        std::snprintf(buf, sizeof(buf),
                      "Charging at %dW (charger can do up to %dW)",
                      activeW, chargerMaxW);
        diag.detail = buf;
        diag.isWarning = false;
    } else {
        diag.bottleneck = Fine;
        std::snprintf(buf, sizeof(buf), "Charging well at %dW",
                      activeW > 0 ? activeW : chargerMaxW);
        diag.summary = buf;
        diag.detail.clear();
        diag.isWarning = false;
    }

    return diag;
}

} // namespace WhatCable
