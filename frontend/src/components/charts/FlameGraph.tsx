import React, { useMemo, useState } from 'react';
import { cn } from '@/lib/utils';

interface FlameNode {
  name: string;
  value: number;
  children?: FlameNode[];
  color?: string;
}

interface FlameGraphProps {
  data: FlameNode;
  height?: number;
}

export default function FlameGraph({ data, height = 400 }: FlameGraphProps) {
  const [hoveredNode, setHoveredNode] = useState<FlameNode | null>(null);

  const totalValue = data.value;
  
  const renderNode = (node: FlameNode, level: number, startX: number) => {
    const width = (node.value / totalValue) * 100;
    if (width < 0.05) return null; // Skip tiny nodes

    return (
      <React.Fragment key={`${node.name}-${level}-${startX}`}>
        <div
          className={cn(
            "absolute border border-black/10 text-[10px] overflow-hidden whitespace-nowrap px-1 cursor-pointer transition-all hover:brightness-110",
            level === 0 ? "bg-orange-500" : "bg-orange-400/80"
          )}
          style={{
            left: `${startX}%`,
            width: `${width}%`,
            bottom: `${level * 20}px`,
            height: '20px',
            backgroundColor: node.color || `hsl(${20 + level * 5}, 70%, 50%)`,
          }}
          onMouseEnter={() => setHoveredNode(node)}
          onMouseLeave={() => setHoveredNode(null)}
          title={`${node.name} (${node.value} samples, ${((node.value / totalValue) * 100).toFixed(1)}%)`}
        >
          {width > 5 && <span className="text-white mix-blend-difference">{node.name}</span>}
        </div>
        {node.children?.map((child, i) => {
          const childStartX = startX + (node.children!.slice(0, i).reduce((sum, c) => sum + c.value, 0) / totalValue) * 100;
          return renderNode(child, level + 1, childStartX);
        })}
      </React.Fragment>
    );
  };

  const maxLevel = useMemo(() => {
    const getDepth = (n: FlameNode): number => {
      if (!n.children || n.children.length === 0) return 1;
      return 1 + Math.max(...n.children.map(getDepth));
    };
    return getDepth(data);
  }, [data]);

  return (
    <div className="relative w-full bg-slate-900/50 rounded-lg border border-slate-700 p-4 overflow-hidden" style={{ height: `${Math.max(height, maxLevel * 20 + 40)}px` }}>
      <div className="relative w-full h-full">
        {renderNode(data, 0, 0)}
      </div>
      
      {hoveredNode && (
        <div className="absolute top-2 right-2 bg-slate-800 border border-slate-700 rounded px-2 py-1 text-[10px] text-slate-300 z-10 shadow-xl pointer-events-none">
          <div className="font-bold text-white">{hoveredNode.name}</div>
          <div>Samples: {hoveredNode.value.toLocaleString()}</div>
          <div>Percentage: {((hoveredNode.value / totalValue) * 100).toFixed(2)}%</div>
        </div>
      )}
    </div>
  );
}
