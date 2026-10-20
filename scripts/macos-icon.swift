#!/usr/bin/env swift
// Génère les icônes macOS de Daftr à partir de assets/logo.svg :
//   - src-tauri/crates/desktop/icons/icon.icns  (tuile « squircle » à la grille Apple Big Sur)
//   - src-tauri/crates/desktop/icons/tray-template.png    (22x22, glyphe noir sur transparent)
//   - src-tauri/crates/desktop/icons/tray-template@2x.png (44x44)
//
// Utilisation (depuis la racine du dépôt, sur macOS) :
//   swift scripts/macos-icon.swift
//
// Nécessite Xcode / les outils en ligne de commande (swift, iconutil). Aucune dépendance externe.
// Les icônes Windows (.ico, PNG) ne sont jamais touchées.
import AppKit

let fm = FileManager.default
let root = URL(fileURLWithPath: fm.currentDirectoryPath)
let iconsDir = root.appendingPathComponent("src-tauri/crates/desktop/icons")
let svgURL = root.appendingPathComponent("assets/logo.svg")

func fail(_ msg: String) -> Never {
    FileHandle.standardError.write(Data("erreur : \(msg)\n".utf8))
    exit(1)
}

// Rend un dessin dans un bitmap de exactement w x h pixels (indépendant de l'échelle de l'écran).
func bitmap(_ w: Int, _ h: Int, _ draw: (CGRect) -> Void) -> NSBitmapImageRep {
    guard let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h, bitsPerSample: 8, samplesPerPixel: 4,
        hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0),
        let ctx = NSGraphicsContext(bitmapImageRep: rep)
    else { fail("création du bitmap impossible") }
    rep.size = NSSize(width: w, height: h)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = ctx
    ctx.imageInterpolation = .high
    draw(CGRect(x: 0, y: 0, width: w, height: h))
    NSGraphicsContext.restoreGraphicsState()
    return rep
}

func writePNG(_ rep: NSBitmapImageRep, _ url: URL) {
    guard let data = rep.representation(using: .png, properties: [:]) else { fail("encodage PNG") }
    do { try data.write(to: url) } catch { fail("écriture de \(url.path) : \(error)") }
}

func svgImage(_ svg: String) -> NSImage {
    guard let img = NSImage(data: Data(svg.utf8)), img.isValid else { fail("SVG illisible") }
    return img
}

// --- Glyphe blanc du logo (sans le carré violet d'origine, remplacé par notre tuile) ---
guard var logoSVG = try? String(contentsOf: svgURL, encoding: .utf8) else { fail("lecture de logo.svg") }
let rectStart = logoSVG.range(of: "<rect ")
let rectEnd = logoSVG.range(of: "/>", range: logoSVG.range(of: "<rect ")!.upperBound..<logoSVG.endIndex)
if let s = rectStart, let e = rectEnd { logoSVG.removeSubrange(s.lowerBound..<e.upperBound) }
let glyph = svgImage(logoSVG)

// --- Icône d'application : tuile 824x824 sur canevas 1024x1024, coins ~185 px ---
let brandTop = NSColor(srgbRed: 0x7A / 255, green: 0x5C / 255, blue: 0xC8 / 255, alpha: 1)
let brandBottom = NSColor(srgbRed: 0x4A / 255, green: 0x30 / 255, blue: 0x8C / 255, alpha: 1) // proche de #5B3FA3

func drawTile(_ r: CGRect) {
    let k = r.width / 1024
    let tile = CGRect(x: 100 * k, y: 100 * k, width: 824 * k, height: 824 * k)
    let path = NSBezierPath(roundedRect: tile, xRadius: 185 * k, yRadius: 185 * k)

    // Ombre portée discrète sous la tuile.
    NSGraphicsContext.saveGraphicsState()
    let shadow = NSShadow()
    shadow.shadowColor = NSColor.black.withAlphaComponent(0.30)
    shadow.shadowOffset = NSSize(width: 0, height: -10 * k)
    shadow.shadowBlurRadius = 20 * k
    shadow.set()
    brandBottom.setFill()
    path.fill()
    NSGraphicsContext.restoreGraphicsState()

    // Dégradé de marque (plus clair en haut).
    NSGraphicsContext.saveGraphicsState()
    path.addClip()
    NSGradient(starting: brandTop, ending: brandBottom)?.draw(in: tile, angle: -90)
    // Fin liseré lumineux intérieur en haut.
    let rim = NSBezierPath(roundedRect: tile.insetBy(dx: 1 * k, dy: 1 * k), xRadius: 184 * k, yRadius: 184 * k)
    rim.lineWidth = 2 * k
    NSColor.white.withAlphaComponent(0.14).setStroke()
    rim.stroke()
    NSGraphicsContext.restoreGraphicsState()

    // Logo centré. Le trait visible n'occupe qu'~54 % du viewBox 512x512 : on étend donc le viewBox
    // à ~95 % de la tuile pour que le glyphe couvre environ la moitié de la tuile.
    let side = tile.width * 0.95
    let box = CGRect(x: tile.midX - side / 2, y: tile.midY - side / 2, width: side, height: side)
    glyph.draw(in: box, from: .zero, operation: .sourceOver, fraction: 1)
}

let tmp = fm.temporaryDirectory.appendingPathComponent("daftr-icon-\(getpid())")
let iconset = tmp.appendingPathComponent("icon.iconset")
try? fm.removeItem(at: tmp)
do { try fm.createDirectory(at: iconset, withIntermediateDirectories: true) } catch { fail("dossier temporaire") }

for base in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let px = base * scale
        let name = scale == 1 ? "icon_\(base)x\(base).png" : "icon_\(base)x\(base)@2x.png"
        writePNG(bitmap(px, px, drawTile), iconset.appendingPathComponent(name))
    }
}

let icns = iconsDir.appendingPathComponent("icon.icns")
let p = Process()
p.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
p.arguments = ["-c", "icns", iconset.path, "-o", icns.path]
do { try p.run() } catch { fail("iconutil introuvable") }
p.waitUntilExit()
if p.terminationStatus != 0 { fail("iconutil a échoué") }
try? fm.removeItem(at: tmp)
print("écrit \(icns.path)")

// --- Icône template de la barre de menus : trait noir plein (macOS la recolore) ---
// Seul le trait principal du logo, épaissi pour rester lisible à 16 px.
let trayGlyph = svgImage("""
<svg xmlns="http://www.w3.org/2000/svg" viewBox="8 8 86 86" width="86" height="86">
<path d="M 58 18 C 63 34 69 50 75 66 Q 78 77 66 77 L 32 77 C 25 77 20 72 20 64" fill="none" stroke="#000000" stroke-width="16" stroke-linecap="round" stroke-linejoin="round"/>
</svg>
""")
for (px, name) in [(22, "tray-template.png"), (44, "tray-template@2x.png")] {
    let rep = bitmap(px, px) { r in
        let m = r.width * 0.09
        trayGlyph.draw(in: r.insetBy(dx: m, dy: m), from: .zero, operation: .sourceOver, fraction: 1)
    }
    writePNG(rep, iconsDir.appendingPathComponent(name))
    print("écrit \(name)")
}
