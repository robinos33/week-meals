import "./screens.css";

const DAY_NAMES = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

/** Renvoie les 7 jours de la semaine courante (à partir de lundi). */
function currentWeek(): { name: string; label: string }[] {
  const today = new Date();
  const monday = new Date(today);
  const offset = (today.getDay() + 6) % 7; // 0 = lundi
  monday.setDate(today.getDate() - offset);
  return DAY_NAMES.map((name, index) => {
    const day = new Date(monday);
    day.setDate(monday.getDate() + index);
    return { name, label: `${day.getDate()}/${day.getMonth() + 1}` };
  });
}

/** Onglet Semaine : 7 jours × 2 créneaux (midi / soir), créneaux vides à remplir. */
export function WeekScreen() {
  const week = currentWeek();

  return (
    <section>
      <header className="screen__header">
        <h1 className="screen__title">Semaine</h1>
        <button className="btn" type="button">
          Liste de courses
        </button>
      </header>

      {week.map((day) => (
        <div className="week-day" key={day.name}>
          <div className="week-day__name">
            {day.name} <span className="muted">{day.label}</span>
          </div>
          <div className="week-day__slots">
            <button className="slot" type="button">
              <span className="slot__label">Midi</span>
              <span>+ Ajouter</span>
            </button>
            <button className="slot" type="button">
              <span className="slot__label">Soir</span>
              <span>+ Ajouter</span>
            </button>
          </div>
        </div>
      ))}
    </section>
  );
}
