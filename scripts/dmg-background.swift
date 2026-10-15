// Draws the DMG window background: swift dmg-background.swift <out.png> <scale>
import AppKit

let args = CommandLine.arguments
let out = args[1]
let scale = CGFloat(Double(args[2]) ?? 1)
let (w, h) = (CGFloat(640), CGFloat(400))

let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(w * scale), pixelsHigh: Int(h * scale),
                           bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                           colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
rep.size = NSSize(width: w, height: h)
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)

// Soft vertical gradient.
NSGradient(starting: NSColor(calibratedRed: 0.97, green: 0.97, blue: 0.99, alpha: 1),
           ending: NSColor(calibratedRed: 0.86, green: 0.88, blue: 0.93, alpha: 1))!
    .draw(in: NSRect(x: 0, y: 0, width: w, height: h), angle: -90)

// A faint row of treemap-coloured tiles along the bottom.
let colors: [(CGFloat, CGFloat, CGFloat)] = [(0, 0, 1), (1, 0, 0), (0, 0.78, 0), (0, 0.86, 0.86), (1, 0, 1), (1, 0.86, 0)]
let widths: [CGFloat] = [190, 140, 110, 80, 70, 50]
var x: CGFloat = 0
for (i, cw) in widths.enumerated() {
    let c = colors[i]
    NSColor(calibratedRed: c.0, green: c.1, blue: c.2, alpha: 0.18).setFill()
    NSRect(x: x, y: 0, width: cw, height: 14).fill()
    x += cw
}

func text(_ s: String, size: CGFloat, weight: NSFont.Weight, y: CGFloat, alpha: CGFloat) {
    let p = NSMutableParagraphStyle(); p.alignment = .center
    NSAttributedString(string: s, attributes: [
        .font: NSFont.systemFont(ofSize: size, weight: weight),
        .foregroundColor: NSColor(calibratedWhite: 0.15, alpha: alpha),
        .paragraphStyle: p,
    ]).draw(in: NSRect(x: 0, y: y, width: w, height: size * 1.5))
}
text("Disk Inventory NG", size: 26, weight: .semibold, y: h - 70, alpha: 0.9)
text("Drag the app onto the Applications folder to install", size: 13, weight: .regular, y: 52, alpha: 0.6)

// Arrow between the two icons (icons centred at x=170 and x=470, y=200 from top).
let arrow = NSBezierPath()
let ay = h - 200
arrow.move(to: NSPoint(x: 262, y: ay)); arrow.line(to: NSPoint(x: 368, y: ay))
arrow.move(to: NSPoint(x: 352, y: ay + 14)); arrow.line(to: NSPoint(x: 370, y: ay)); arrow.line(to: NSPoint(x: 352, y: ay - 14))
arrow.lineWidth = 6; arrow.lineCapStyle = .round; arrow.lineJoinStyle = .round
NSColor(calibratedWhite: 0.35, alpha: 0.55).setStroke()
arrow.stroke()

NSGraphicsContext.current = nil
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
