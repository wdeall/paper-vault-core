import { useParams } from "react-router-dom";
import { ReaderShell } from "@/components/reader/ReaderShell";

export function ReaderPage() {
  const { id } = useParams<{ id: string }>();
  if (!id) {
    return (
      <div className="flex h-screen items-center justify-center text-muted-foreground">
        未指定论文
      </div>
    );
  }
  return <ReaderShell paperId={id} />;
}
