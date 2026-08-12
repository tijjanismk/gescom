import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";

interface PaginationProps {
  page: number;       // 0-based
  total: number;      // nombre total d'éléments
  limite: number;     // éléments par page
  onChanger: (page: number) => void;
}

export function Pagination({ page, total, limite, onChanger }: PaginationProps) {
  const totalPages = Math.ceil(total / limite);
  const debut = page * limite + 1;
  const fin = Math.min((page + 1) * limite, total);

  if (total === 0) return null;

  return (
    <div className="flex items-center justify-between px-2 py-3 border-t border-border">
      <p className="text-xs text-muted-foreground">
        {debut}–{fin} sur {total}
      </p>
      <div className="flex items-center gap-1">
        <Button
          variant="outline" size="sm"
          onClick={() => onChanger(page - 1)}
          disabled={page === 0}
          className="h-7 w-7 p-0"
        >
          <ChevronLeft className="h-4 w-4" />
        </Button>

        {/* Pages visibles — max 5 autour de la page courante */}
        {Array.from({ length: totalPages }, (_, i) => i)
          .filter(i =>
            i === 0 ||
            i === totalPages - 1 ||
            Math.abs(i - page) <= 1
          )
          .reduce<(number | "...")[]>((acc, i, idx, arr) => {
            if (idx > 0 && i - (arr[idx - 1] as number) > 1) acc.push("...");
            acc.push(i);
            return acc;
          }, [])
          .map((item, idx) =>
            item === "..." ? (
              <span key={`dots-${idx}`} className="text-xs text-muted-foreground px-1">…</span>
            ) : (
              <Button
                key={item}
                variant={page === item ? "default" : "outline"}
                size="sm"
                onClick={() => onChanger(item as number)}
                className="h-7 w-7 p-0 text-xs"
              >
                {(item as number) + 1}
              </Button>
            )
          )
        }

        <Button
          variant="outline" size="sm"
          onClick={() => onChanger(page + 1)}
          disabled={page >= totalPages - 1}
          className="h-7 w-7 p-0"
        >
          <ChevronRight className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
