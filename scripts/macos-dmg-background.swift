#!/usr/bin/env swift
// Génère le fond du DMG macOS de Daftr : assets/dmg-background.png.
//
// Utilisation (depuis la racine du dépôt, sur macOS) :
//   swift scripts/macos-dmg-background.swift
//
// L'image fait 1320x800 px avec une densité de 144 dpi, soit 660x400 pt à l'affichage
// (Retina). Les positions ci-dessous (en points, origine en haut à gauche) doivent rester
// alignées avec bundle.macOS.dmg de src-tauri/crates/desktop/tauri.macos.conf.json :
//   windowSize 660x400, appPosition {180,170}, applicationFolderPosition {480,170}.
import AppKit

let fm = FileManager.default
let root = URL(fileURLWithPath: fm.currentDirectoryPath)
let out = root.appendingPathComponent("assets/dmg-background.png")

func fail(_ msg: String) -> Never {
    FileHandle.standardError.write(Data("erreur : \(msg)\n".utf8))
    exit(1)
}

let widthPt = 660.0, heightPt = 400.0, scale = 2.0
let appX = 180.0, folderX = 480.0, iconY = 170.0   // centres des icônes, en points (y depuis le haut)

guard var logotype = try? String(contentsOf: root.appendingPathComponent("assets/logotype.svg"), encoding: .utf8)
else { fail("lecture de logotype.svg") }
// Le logotype est blanc (currentColor) : on le passe en gris-violet foncé pour le fond clair.
logotype = logotype.replacingOccurrences(of: "color=\"#ffffff\"", with: "color=\"#5B3FA3\"")
guard let logo = NSImage(data: Data(logotype.utf8)), logo.isValid else { fail("logotype illisible") }

guard let rep = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: Int(widthPt * scale), pixelsHigh: Int(heightPt * scale),
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0),
    let ctx = NSGraphicsContext(bitmapImageRep: rep)
else { fail("création du bitmap impossible") }

NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = ctx
ctx.imageInterpolation = .high
// On dessine en points : 1 pt = 2 px.
ctx.cgContext.scaleBy(x: scale, y: scale)

// Fond uni clair.
NSColor(srgbRed: 0xF5 / 255, green: 0xF5 / 255, blue: 0xF7 / 255, alpha: 1).setFill()
CGRect(x: 0, y: 0, width: widthPt, height: heightPt).fill()

// Conversion y « depuis le haut » → coordonnées AppKit (origine en bas).
func flipY(_ y: Double) -> Double { heightPt - y }

// Logotype centré en haut (hauteur 44 pt).
let logoH = 44.0
let logoW = logoH * 591 / 168
logo.draw(in: CGRect(x: (widthPt - logoW) / 2, y: flipY(30) - logoH, width: logoW, height: logoH),
          from: .zero, operation: .sourceOver, fraction: 1)

// Flèche entre l'app et le dossier Applications (les icônes font ~128 pt de large : on laisse de la marge).
let arrowColor = NSColor(srgbRed: 0x8E / 255, green: 0x8E / 255, blue: 0x93 / 255, alpha: 1)
let y = flipY(iconY)
let x0 = appX + 80, x1 = folderX - 80
let shaft = NSBezierPath()
shaft.move(to: CGPoint(x: x0, y: y))
shaft.line(to: CGPoint(x: x1 - 4, y: y))
shaft.lineWidth = 5
shaft.lineCapStyle = .round
arrowColor.setStroke()
shaft.stroke()
let head = NSBezierPath()
head.move(to: CGPoint(x: x1 - 16, y: y + 14))
head.line(to: CGPoint(x: x1, y: y))
head.line(to: CGPoint(x: x1 - 16, y: y - 14))
head.lineWidth = 5
head.lineCapStyle = .round
head.lineJoinStyle = .round
head.stroke()

// Texte discret en bas.
let style = NSMutableParagraphStyle()
style.alignment = .center
let text = NSAttributedString(string: "Drag Daftr to Applications", attributes: [
    .font: NSFont.systemFont(ofSize: 13, weight: .regular),
    .foregroundColor: NSColor(srgbRed: 0x6E / 255, green: 0x6E / 255, blue: 0x73 / 255, alpha: 1),
    .paragraphStyle: style,
])
text.draw(in: CGRect(x: 0, y: flipY(350), width: widthPt, height: 20))

NSGraphicsContext.restoreGraphicsState()

guard let data = rep.representation(using: .png, properties: [:]) else { fail("encodage PNG") }
do { try data.write(to: out) } catch { fail("écriture de \(out.path)") }

// Densité 144 dpi pour un affichage 660x400 pt.
let p = Process()
p.executableURL = URL(fileURLWithPath: "/usr/bin/sips")
p.arguments = ["-s", "dpiHeight", "144", "-s", "dpiWidth", "144", out.path]
p.standardOutput = FileHandle.nullDevice
do { try p.run() } catch { fail("sips introuvable") }
p.waitUntilExit()
if p.terminationStatus != 0 { fail("sips a échoué") }
print("écrit \(out.path)")
