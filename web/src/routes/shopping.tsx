import "./screens.css";

/** Onglet Courses : ajout rapide en haut, état vide, indicateur de sync (stub). */
export function ShoppingScreen() {
  return (
    <section>
      <header className="screen__header">
        <h1 className="screen__title">Courses</h1>
        <span className="sync-badge" aria-live="polite">
          <span aria-hidden="true">●</span> à jour
        </span>
      </header>

      <form className="quick-add" onSubmit={(e) => e.preventDefault()}>
        <input
          className="input"
          placeholder="Ajouter un article…"
          aria-label="Ajouter un article"
        />
        <button className="btn btn--primary" type="submit">
          Ajouter
        </button>
      </form>

      <div className="empty-state">
        <div className="empty-state__emoji">🛒</div>
        <h2>Liste vide</h2>
        <p>Générez-la depuis la semaine, ou ajoutez un article ci-dessus.</p>
      </div>
    </section>
  );
}
