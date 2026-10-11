type Props = { folder: string | null; onChoose?: () => void; pending?: boolean };
export function WatchedFolder({ folder, onChoose, pending = false }: Props) {
  return <section className="panel" aria-labelledby="folder-heading">
    <div className="section-header"><h2 id="folder-heading">Watched folder</h2><button onClick={onChoose} disabled={!onChoose || pending}>{pending ? "Choosing…" : "Choose folder"}</button></div>
    <p className="folder-path">{folder ?? "Choose a screenshot folder to get started."}</p>
  </section>;
}
