// Draws the app icon: a mouse on a rounded square. Run by `make icon`, which turns the PNG into
// macos/AppIcon.icns. Usage: swift icon.swift out.png
import AppKit

let size = 1024.0
// Draw into an exact 1024px bitmap; NSImage.lockFocus would double it on a Retina screen.
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(size), pixelsHigh: Int(size),
                           bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                           colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)

// Background tile on the macOS icon grid: 824pt square, inset 100pt, with a soft drop shadow.
let tile = NSBezierPath(roundedRect: NSRect(x: 100, y: 100, width: 824, height: 824), xRadius: 185, yRadius: 185)
NSGraphicsContext.saveGraphicsState()
let shadow = NSShadow()
shadow.shadowColor = NSColor.black.withAlphaComponent(0.3)
shadow.shadowOffset = NSSize(width: 0, height: -12)
shadow.shadowBlurRadius = 24
shadow.set()
NSColor.black.setFill()
tile.fill()
NSGraphicsContext.restoreGraphicsState()
NSGradient(starting: NSColor(red: 0.20, green: 0.24, blue: 0.33, alpha: 1),
           ending: NSColor(red: 0.07, green: 0.08, blue: 0.12, alpha: 1))!.draw(in: tile, angle: -90)

// Mouse body.
let body = NSRect(x: 362, y: 262, width: 300, height: 480)
let mouse = NSBezierPath(roundedRect: body, xRadius: 150, yRadius: 150)
NSGradient(starting: NSColor(white: 0.98, alpha: 1), ending: NSColor(white: 0.80, alpha: 1))!
    .draw(in: mouse, angle: -90)

// Button split from the top down to the wheel.
let split = NSBezierPath()
split.move(to: NSPoint(x: 512, y: body.maxY))
split.line(to: NSPoint(x: 512, y: 680))
split.lineWidth = 10
NSColor(white: 0.55, alpha: 1).setStroke()
split.stroke()

// Scroll wheel in Logitech-ish teal.
NSColor(red: 0.0, green: 0.78, blue: 0.70, alpha: 1).setFill()
NSBezierPath(roundedRect: NSRect(x: 492, y: 580, width: 40, height: 100), xRadius: 20, yRadius: 20).fill()

NSGraphicsContext.current = nil
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
