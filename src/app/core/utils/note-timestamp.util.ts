// SQLite stores `datetime('now')` as "YYYY-MM-DD HH:MM:SS" in UTC, without any timezone marker
const SQLITE_UTC_DATETIME = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/;

/** Parses a note date from the database; invalid input gives an Invalid Date. */
export function parseNoteDate(value: string): Date {
  return new Date(SQLITE_UTC_DATETIME.test(value) ? `${value.replace(' ', 'T')}Z` : value);
}

function pad(value: number): string {
  return String(value).padStart(2, '0');
}

/** "dd/MM/yyyy HH:mm:ss" in local time; empty when the date is unusable. */
export function formatNoteTimestamp(value: string): string {
  const date = parseNoteDate(value);
  if (isNaN(date.getTime())) {
    return '';
  }
  const day = `${pad(date.getDate())}/${pad(date.getMonth() + 1)}/${date.getFullYear()}`;
  const time = `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
  return `${day} ${time}`;
}
