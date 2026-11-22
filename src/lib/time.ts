const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** Short relative age as the graph shows it: "now", "5 min", "3 h", "23 Oct", "23 Oct 2024". */
export function age(seconds: number, now = Date.now() / 1000): string {
  const diff = now - seconds;
  if (diff < 60) {
    return "now";
  }
  if (diff < 3600) {
    return `${Math.floor(diff / 60)} min`;
  }
  if (diff < 86400) {
    return `${Math.floor(diff / 3600)} h`;
  }
  const d = new Date(seconds * 1000);
  const day = `${d.getDate()} ${MONTHS[d.getMonth()]}`;
  return d.getFullYear() === new Date(now * 1000).getFullYear() ? day : `${day} ${d.getFullYear()}`;
}

/** Local "HH:MM". */
export function clock(seconds: number): string {
  const d = new Date(seconds * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** Local "YYYY-MM-DD HH:MM", for tooltips. */
export function fullDate(seconds: number): string {
  const d = new Date(seconds * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${clock(seconds)}`;
}
