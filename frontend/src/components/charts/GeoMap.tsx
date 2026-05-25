interface GeoMapPoint {
  lat: number
  lng: number
  label: string
  value: number
  color?: string
}

interface GeoMapProps {
  points: GeoMapPoint[]
  height?: number
  maxBubbleSize?: number
}

export default function GeoMap({ points, height = 400, maxBubbleSize = 30 }: GeoMapProps) {
  const maxVal = Math.max(...points.map(p => p.value), 1)

  return (
    <div style={{ height }} className="relative bg-slate-800 rounded-lg overflow-hidden border border-slate-700">
      {/* Simple SVG world map placeholder — replace with Leaflet when react-leaflet is added */}
      <svg viewBox="-180 -90 360 180" className="w-full h-full" preserveAspectRatio="xMidYMid meet">
        {/* Simple continent outlines */}
        <rect x="-180" y="-90" width="360" height="180" fill="#1e293b" />
        {/* Grid lines */}
        {[-60, -30, 0, 30, 60].map(lat => (
          <line key={`lat${lat}`} x1="-180" y1={-lat} x2="180" y2={-lat} stroke="#334155" strokeWidth="0.3" />
        ))}
        {[-120, -60, 0, 60, 120].map(lng => (
          <line key={`lng${lng}`} x1={lng} y1="-90" x2={lng} y2="90" stroke="#334155" strokeWidth="0.3" />
        ))}
        {/* Simplified continent shapes */}
        <path d="M-130,-10 L-120,10 L-100,20 L-80,30 L-60,25 L-40,-5 L-60,-15 L-80,-10 Z" fill="#334155" opacity="0.5" />
        <path d="M-20,-40 L10,-35 L30,-30 L50,-20 L40,10 L20,30 L-10,35 L-20,10 Z" fill="#334155" opacity="0.5" />
        <path d="M20,-10 L40,-5 L60,5 L80,15 L100,10 L120,-5 L140,-20 L130,-40 L100,-50 L60,-45 L30,-35 Z" fill="#334155" opacity="0.5" />
        <path d="M100,10 L120,5 L140,-10 L150,-30 L130,-40 L110,-30 Z" fill="#334155" opacity="0.5" />

        {/* Data points */}
        {points.map((p, i) => {
          const size = Math.max(3, (p.value / maxVal) * maxBubbleSize)
          const color = p.color || '#3b82f6'
          return (
            <g key={i}>
              <circle cx={p.lng} cy={-p.lat} r={size} fill={color} fillOpacity={0.6} stroke={color} strokeWidth={0.5}>
                <title>{`${p.label}: ${p.value.toLocaleString()}`}</title>
              </circle>
              <circle cx={p.lng} cy={-p.lat} r={size * 0.4} fill={color} fillOpacity={0.9} />
            </g>
          )
        })}
      </svg>

      {/* Legend */}
      <div className="absolute bottom-2 right-2 bg-slate-900/80 border border-slate-700 rounded px-2 py-1">
        <p className="text-[9px] text-slate-400">{points.length} locations</p>
      </div>
    </div>
  )
}
