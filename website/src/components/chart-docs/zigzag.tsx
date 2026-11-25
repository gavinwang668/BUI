import React from "react"

interface TriangleLineProps extends React.HTMLAttributes<SVGSVGElement> {
  /** Height of the zig-zag pattern in pixels. Defaults to 24. */
  height?: number
  /** Width of a single complete zig-zag peak cycle in pixels. Defaults to 32. */
  peakWidth?: number
  /** Thickness of the line stroke in pixels. Defaults to 3. */
  strokeWidth?: number
}

export const TriangleLine: React.FC<TriangleLineProps> = ({ height = 24, peakWidth = 32, strokeWidth = 3, className, ...props }) => {
  const halfWidth = peakWidth / 2

  return (
    <svg className={`w-full text-primary ${className ?? ""}`} style={{ height: `${height + strokeWidth}px` }} xmlns="http://w3.org" {...props}>
      <defs>
        <pattern id="zigzag-pattern" width={peakWidth} height={height + strokeWidth} patternUnits="userSpaceOnUse">
          <polyline
            points={`0,${height} ${halfWidth},${strokeWidth / 2} ${peakWidth},${height}`}
            fill="none"
            stroke="currentColor"
            strokeWidth={strokeWidth}
            strokeLinecap="square"
            strokeLinejoin="miter"
          />
        </pattern>
      </defs>
      <rect width="100%" height="100%" fill="url(#zigzag-pattern)" />
    </svg>
  )
}

export default TriangleLine
