#include "UsbClassDB.h"
#include <cstdio>

namespace WhatCable {

std::string UsbClassDB::className(uint8_t classCode)
{
    switch (classCode) {
    case 0x00: return "Composite";
    case 0x01: return "Audio";
    case 0x02: return "Communications";
    case 0x03: return "HID";
    case 0x05: return "Physical";
    case 0x06: return "Image";
    case 0x07: return "Printer";
    case 0x08: return "Mass Storage";
    case 0x09: return "Hub";
    case 0x0A: return "CDC Data";
    case 0x0B: return "Smart Card";
    case 0x0D: return "Content Security";
    case 0x0E: return "Video";
    case 0x0F: return "Personal Healthcare";
    case 0x10: return "Audio/Video";
    case 0x11: return "Billboard";
    case 0x12: return "USB Type-C Bridge";
    case 0xDC: return "Diagnostic";
    case 0xE0: return "Wireless";
    case 0xEF: return "Miscellaneous";
    case 0xFE: return "Application Specific";
    case 0xFF: return "Vendor Specific";
    default: {
        char buf[16];
        std::snprintf(buf, sizeof(buf), "0x%02x", classCode);
        return buf;
    }
    }
}

std::string UsbClassDB::interfaceClassName(uint8_t classCode, uint8_t subClass)
{
    if (classCode == 0x01) {
        switch (subClass) {
        case 0x01: return "Audio Control";
        case 0x02: return "Audio Streaming";
        case 0x03: return "MIDI Streaming";
        }
    }
    if (classCode == 0x03) {
        switch (subClass) {
        case 0x00: return "HID (No Subclass)";
        case 0x01: return "HID Boot Interface";
        }
    }
    if (classCode == 0x08) {
        switch (subClass) {
        case 0x01: return "RBC";
        case 0x02: return "MMC-5 (ATAPI)";
        case 0x04: return "UFI";
        case 0x06: return "SCSI";
        case 0x08: return "UAS";
        }
    }
    if (classCode == 0x0E) {
        switch (subClass) {
        case 0x01: return "Video Control";
        case 0x02: return "Video Streaming";
        }
    }
    if (classCode == 0xE0) {
        switch (subClass) {
        case 0x01: return "Bluetooth";
        case 0x02: return "Wireless USB";
        }
    }
    return className(classCode);
}

} // namespace WhatCable
