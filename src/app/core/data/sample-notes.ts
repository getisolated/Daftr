import { Language } from '../i18n/language.model';

interface SampleNote {
  title: string;
  content: string;
}

/** English sample notes; their titles are also how `gatherLegacySampleNotes` recognises notes seeded long ago. */
export const SAMPLE_NOTES: SampleNote[] = [
  {
    title: 'Markdown Basics',
    content: `# Main Heading H1

## Subheading H2

### Section H3

#### Subsection H4

This is a regular paragraph with **bold text**, *italic text*, and ~~strikethrough~~. You can also combine ***bold and italic***.

Here is a [link to Google](https://google.com) and some \`inline code\` in a sentence.

---

> This is a blockquote.
> It can span multiple lines.

Unordered list:
- First item
- Second item
- Third item

Ordered list:
1. Step one
2. Step two
3. Step three`,
  },
  {
    title: 'Code Blocks',
    content: `# Code Blocks

Here is some TypeScript:

\`\`\`typescript
interface User {
  id: number;
  name: string;
  email?: string;
}

const getUser = async (id: number): Promise<User | null> => {
  if (id <= 0) return null;
  const response = await fetch(\\\`/api/users/\\\${id}\\\`);
  return response.json();
};

export class UserService {
  private users: Map<number, User> = new Map();

  findById(id: number): User | undefined {
    return this.users.get(id);
  }
}
\`\`\`

Some Python:

\`\`\`python
def fibonacci(n: int) -> list[int]:
    """Generate fibonacci sequence up to n terms."""
    if n <= 0:
        return []
    sequence = [0, 1]
    for i in range(2, n):
        sequence.append(sequence[-1] + sequence[-2])
    return sequence[:n]

# Usage
result = fibonacci(10)
print(f"Fibonacci: {result}")  # [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]
\`\`\`

Some SQL:

\`\`\`sql
SELECT u.name, COUNT(o.id) AS total_orders
FROM users u
LEFT JOIN orders o ON o.user_id = u.id
WHERE u.created_at >= '2025-01-01'
GROUP BY u.name
HAVING COUNT(o.id) > 5
ORDER BY total_orders DESC;
\`\`\``,
  },
  {
    title: 'Project Roadmap',
    content: `# Project Roadmap

## Current Sprint

- [x] Implement the Markdown editor
- [x] Add the command palette
- [ ] Add image support
- [ ] Export to PDF

## Bugs to Fix

- ( ) Startup crash on Linux
- (x) Fix scroll in the notes list
- ( ) FTS search doesn't match accented characters

## Notes

The \`( )\` and \`(x)\` format uses round bullets instead of checkboxes.

While \`[ ]\` and \`[x]\` renders actual clickable **checkboxes**.

Priorities: \`high\` > \`medium\` > \`low\``,
  },
  {
    title: 'Contributing Guide',
    content: `# Contributing Guide

## Prerequisites

Install dependencies with:

\`\`\`bash
npm install
npm run electron:rebuild
\`\`\`

> **Important**: Don't forget \`electron:rebuild\` after each \`npm install\`, otherwise \`better-sqlite3\` won't work.

## Architecture

The project follows this structure:

| Folder | Role |
|--------|------|
| \`electron/\` | Main process |
| \`src/app/core/\` | Angular services |
| \`src/app/features/\` | UI components |

## PR Checklist

- [ ] Code compiles without errors
- [ ] No leftover \`console.log\`
- [x] Tests pass
- [x] Lint is clean

## Service Example

\`\`\`typescript
@Injectable({ providedIn: 'root' })
export class NotesService {
  private readonly _notes = signal<Note[]>([]);

  readonly notes = this._notes.asReadonly();

  async loadAll(): Promise<void> {
    const notes = await this.bridge.getAllNotes();
    this._notes.set(notes);
  }
}
\`\`\``,
  },
];

/** French sample notes (same notes, same markdown syntax). */
export const SAMPLE_NOTES_FR: SampleNote[] = [
  {
    title: 'Bases du Markdown',
    content: `# Titre principal H1

## Sous-titre H2

### Section H3

#### Sous-section H4

Ceci est un paragraphe classique avec du **texte en gras**, du *texte en italique* et du ~~texte barré~~. Vous pouvez aussi combiner ***gras et italique***.

Voici un [lien vers Google](https://google.com) et du \`code en ligne\` dans une phrase.

---

> Ceci est une citation.
> Elle peut s'étendre sur plusieurs lignes.

Liste non ordonnée :
- Premier élément
- Deuxième élément
- Troisième élément

Liste ordonnée :
1. Étape un
2. Étape deux
3. Étape trois`,
  },
  {
    title: 'Blocs de code',
    content: `# Blocs de code

Voici du TypeScript :

\`\`\`typescript
interface User {
  id: number;
  name: string;
  email?: string;
}

const getUser = async (id: number): Promise<User | null> => {
  if (id <= 0) return null;
  const response = await fetch(\\\`/api/users/\\\${id}\\\`);
  return response.json();
};

export class UserService {
  private users: Map<number, User> = new Map();

  findById(id: number): User | undefined {
    return this.users.get(id);
  }
}
\`\`\`

Un peu de Python :

\`\`\`python
def fibonacci(n: int) -> list[int]:
    """Génère la suite de Fibonacci jusqu'à n termes."""
    if n <= 0:
        return []
    sequence = [0, 1]
    for i in range(2, n):
        sequence.append(sequence[-1] + sequence[-2])
    return sequence[:n]

# Utilisation
result = fibonacci(10)
print(f"Fibonacci: {result}")  # [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]
\`\`\`

Un peu de SQL :

\`\`\`sql
SELECT u.name, COUNT(o.id) AS total_orders
FROM users u
LEFT JOIN orders o ON o.user_id = u.id
WHERE u.created_at >= '2025-01-01'
GROUP BY u.name
HAVING COUNT(o.id) > 5
ORDER BY total_orders DESC;
\`\`\``,
  },
  {
    title: 'Feuille de route du projet',
    content: `# Feuille de route du projet

## Sprint en cours

- [x] Implémenter l'éditeur Markdown
- [x] Ajouter la palette de commandes
- [ ] Ajouter la prise en charge des images
- [ ] Exporter en PDF

## Bugs à corriger

- ( ) Plantage au démarrage sous Linux
- (x) Corriger le défilement dans la liste des notes
- ( ) La recherche FTS ne trouve pas les caractères accentués

## Notes

Le format \`( )\` et \`(x)\` utilise des puces rondes au lieu de cases à cocher.

Alors que \`[ ]\` et \`[x]\` affichent de véritables **cases à cocher** cliquables.

Priorités : \`high\` > \`medium\` > \`low\``,
  },
  {
    title: 'Guide de contribution',
    content: `# Guide de contribution

## Prérequis

Installez les dépendances avec :

\`\`\`bash
npm install
npm run electron:rebuild
\`\`\`

> **Important** : n'oubliez pas \`electron:rebuild\` après chaque \`npm install\`, sinon \`better-sqlite3\` ne fonctionnera pas.

## Architecture

Le projet suit cette structure :

| Dossier | Rôle |
|---------|------|
| \`electron/\` | Processus principal |
| \`src/app/core/\` | Services Angular |
| \`src/app/features/\` | Composants d'interface |

## Checklist de PR

- [ ] Le code compile sans erreur
- [ ] Aucun \`console.log\` oublié
- [x] Les tests passent
- [x] Le lint est propre

## Exemple de service

\`\`\`typescript
@Injectable({ providedIn: 'root' })
export class NotesService {
  private readonly _notes = signal<Note[]>([]);

  readonly notes = this._notes.asReadonly();

  async loadAll(): Promise<void> {
    const notes = await this.bridge.getAllNotes();
    this._notes.set(notes);
  }
}
\`\`\``,
  },
];

/** Sample notes created on first launch, in the language of the app. */
export function getSampleNotes(language: Language): SampleNote[] {
  return language === 'fr' ? SAMPLE_NOTES_FR : SAMPLE_NOTES;
}
