// Renders the MindFlow app icon to a 1024×1024 PNG: swift scripts/make-icon.swift <out.png>
import AppKit

let size: CGFloat = 1024
let output = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "AppIcon.png"

let rep = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: Int(size), pixelsHigh: Int(size),
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)

// macOS icon grid: 824pt body centred in the 1024 canvas.
let body = NSRect(x: 100, y: 100, width: 824, height: 824)
let squircle = NSBezierPath(roundedRect: body, xRadius: 185, yRadius: 185)

NSGraphicsContext.saveGraphicsState()
let shadow = NSShadow()
shadow.shadowColor = NSColor.black.withAlphaComponent(0.25)
shadow.shadowBlurRadius = 24
shadow.shadowOffset = NSSize(width: 0, height: -10)
shadow.set()
NSColor.white.setFill()
squircle.fill()
NSGraphicsContext.restoreGraphicsState()

let gradient = NSGradient(colors: [
    NSColor(calibratedRed: 0.20, green: 0.74, blue: 0.53, alpha: 1),
    NSColor(calibratedRed: 0.08, green: 0.52, blue: 0.47, alpha: 1),
])!
gradient.draw(in: squircle, angle: -60)

// Three stacked memo cards.
func card(_ rect: NSRect, alpha: CGFloat) {
    NSColor.white.withAlphaComponent(alpha).setFill()
    NSBezierPath(roundedRect: rect, xRadius: 44, yRadius: 44).fill()
}
card(NSRect(x: 300, y: 560, width: 470, height: 200), alpha: 0.35)
card(NSRect(x: 270, y: 430, width: 500, height: 220), alpha: 0.6)
card(NSRect(x: 240, y: 270, width: 540, height: 250), alpha: 1)

// Text lines and a tag on the front card.
let ink = NSColor(calibratedRed: 0.08, green: 0.45, blue: 0.40, alpha: 1)
ink.withAlphaComponent(0.85).setFill()
NSBezierPath(roundedRect: NSRect(x: 290, y: 455, width: 330, height: 34), xRadius: 17, yRadius: 17).fill()
ink.withAlphaComponent(0.45).setFill()
NSBezierPath(roundedRect: NSRect(x: 290, y: 400, width: 420, height: 30), xRadius: 15, yRadius: 15).fill()
let hash = NSAttributedString(string: "#", attributes: [
    .font: NSFont.systemFont(ofSize: 96, weight: .heavy),
    .foregroundColor: ink,
])
hash.draw(at: NSPoint(x: 284, y: 262))

// AI sparkle.
let sparkle = NSBezierPath()
let c = NSPoint(x: 735, y: 760)
let (r1, r2): (CGFloat, CGFloat) = (92, 22)
for i in 0..<8 {
    let angle = CGFloat(i) * .pi / 4 + .pi / 2
    let r = i % 2 == 0 ? r1 : r2
    let p = NSPoint(x: c.x + cos(angle) * r, y: c.y + sin(angle) * r)
    i == 0 ? sparkle.move(to: p) : sparkle.line(to: p)
}
sparkle.close()
NSColor(calibratedRed: 1.0, green: 0.86, blue: 0.35, alpha: 1).setFill()
sparkle.fill()

NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: output))
print("wrote \(output)")
