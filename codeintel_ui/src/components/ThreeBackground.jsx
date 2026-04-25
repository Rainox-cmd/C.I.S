import { useEffect, useRef } from 'react'
import * as THREE from 'three'

const PARTICLE_COUNT = 80
const CONNECTION_DIST = 120
const SPEED = 0.00015

export default function ThreeBackground() {
  const mountRef = useRef(null)

  useEffect(() => {
    const el = mountRef.current
    if (!el) return

    // ── Renderer ──────────────────────────────────────────
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true })
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    renderer.setSize(el.clientWidth, el.clientHeight)
    renderer.setClearColor(0x000000, 0)
    el.appendChild(renderer.domElement)

    // ── Scene + Camera ────────────────────────────────────
    const scene  = new THREE.Scene()
    const camera = new THREE.PerspectiveCamera(60, el.clientWidth / el.clientHeight, 0.1, 1000)
    camera.position.z = 300

    // ── Particles ─────────────────────────────────────────
    const particles = Array.from({ length: PARTICLE_COUNT }, () => ({
      x: (Math.random() - 0.5) * el.clientWidth * 1.4,
      y: (Math.random() - 0.5) * el.clientHeight * 1.4,
      z: (Math.random() - 0.5) * 200,
      vx: (Math.random() - 0.5) * 0.3,
      vy: (Math.random() - 0.5) * 0.3,
      vz: (Math.random() - 0.5) * 0.1,
      size: Math.random() * 2 + 1,
      color: Math.random() > 0.5 ? 0x7c5cfc : 0x00d9ff,
    }))

    // Geometry for nodes
    const nodeGeo = new THREE.SphereGeometry(1, 8, 8)

    const nodeMeshes = particles.map(p => {
      const mat  = new THREE.MeshBasicMaterial({ color: p.color, transparent: true, opacity: 0.7 })
      const mesh = new THREE.Mesh(new THREE.SphereGeometry(p.size, 8, 8), mat)
      mesh.position.set(p.x, p.y, p.z)
      scene.add(mesh)
      return mesh
    })

    // Line segments for connections
    const lineGeometry = new THREE.BufferGeometry()
    const maxLines = PARTICLE_COUNT * PARTICLE_COUNT
    const linePositions = new Float32Array(maxLines * 6)
    const lineColors    = new Float32Array(maxLines * 6)
    lineGeometry.setAttribute('position', new THREE.BufferAttribute(linePositions, 3))
    lineGeometry.setAttribute('color',    new THREE.BufferAttribute(lineColors, 3))

    const lineMat = new THREE.LineSegments(lineGeometry, new THREE.LineBasicMaterial({
      vertexColors: true,
      transparent: true,
      opacity: 0.35,
      linewidth: 1,
    }))
    scene.add(lineMat)

    // Mouse parallax
    let mouseX = 0, mouseY = 0
    const onMouse = (e) => {
      mouseX = (e.clientX / window.innerWidth  - 0.5) * 60
      mouseY = (e.clientY / window.innerHeight - 0.5) * 60
    }
    window.addEventListener('mousemove', onMouse)

    // ── Animation loop ────────────────────────────────────
    let frameId
    let lineCount = 0

    const animate = () => {
      frameId = requestAnimationFrame(animate)

      // Move particles
      particles.forEach((p, i) => {
        p.x += p.vx
        p.y += p.vy
        p.z += p.vz
        const hw = el.clientWidth * 0.7
        const hh = el.clientHeight * 0.7
        if (Math.abs(p.x) > hw) p.vx *= -1
        if (Math.abs(p.y) > hh) p.vy *= -1
        if (Math.abs(p.z) > 100) p.vz *= -1
        nodeMeshes[i].position.set(p.x, p.y, p.z)
      })

      // Update connections
      lineCount = 0
      for (let i = 0; i < particles.length; i++) {
        for (let j = i + 1; j < particles.length; j++) {
          const dx = particles[i].x - particles[j].x
          const dy = particles[i].y - particles[j].y
          const dz = particles[i].z - particles[j].z
          const dist = Math.sqrt(dx * dx + dy * dy + dz * dz)
          if (dist < CONNECTION_DIST) {
            const alpha = 1 - dist / CONNECTION_DIST
            const base  = lineCount * 6
            linePositions[base]   = particles[i].x
            linePositions[base+1] = particles[i].y
            linePositions[base+2] = particles[i].z
            linePositions[base+3] = particles[j].x
            linePositions[base+4] = particles[j].y
            linePositions[base+5] = particles[j].z
            // Color: violet → cyan gradient based on index parity
            const r = 0.48 * alpha, g = 0.36 * alpha, b = alpha
            lineColors[base]   = r; lineColors[base+1] = g; lineColors[base+2] = b
            lineColors[base+3] = 0; lineColors[base+4] = 0.85 * alpha; lineColors[base+5] = alpha
            lineCount++
          }
        }
      }
      lineGeometry.setDrawRange(0, lineCount * 2)
      lineGeometry.attributes.position.needsUpdate = true
      lineGeometry.attributes.color.needsUpdate    = true

      // Camera parallax
      camera.position.x += (mouseX - camera.position.x) * 0.03
      camera.position.y += (-mouseY - camera.position.y) * 0.03
      camera.lookAt(scene.position)

      renderer.render(scene, camera)
    }
    animate()

    // ── Resize ────────────────────────────────────────────
    const onResize = () => {
      const w = el.clientWidth, h = el.clientHeight
      camera.aspect = w / h
      camera.updateProjectionMatrix()
      renderer.setSize(w, h)
    }
    window.addEventListener('resize', onResize)

    // ── Cleanup ───────────────────────────────────────────
    return () => {
      cancelAnimationFrame(frameId)
      window.removeEventListener('mousemove', onMouse)
      window.removeEventListener('resize', onResize)
      renderer.dispose()
      if (el.contains(renderer.domElement)) el.removeChild(renderer.domElement)
    }
  }, [])

  return (
    <div
      ref={mountRef}
      style={{ position: 'fixed', inset: 0, zIndex: 0, pointerEvents: 'none' }}
    />
  )
}
