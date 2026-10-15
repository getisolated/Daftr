"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.folderOps = exports.noteOps = exports.MAX_FOLDER_NAME_LENGTH = void 0;
exports.initDatabase = initDatabase;
exports.closeDatabase = closeDatabase;
exports.sanitizeFtsQuery = sanitizeFtsQuery;
exports.escapeLike = escapeLike;
exports.normalizeFolderName = normalizeFolderName;
const better_sqlite3_1 = __importDefault(require("better-sqlite3"));
const path = __importStar(require("path"));
const main_1 = require("electron/main");
exports.MAX_FOLDER_NAME_LENGTH = 100;
const DEFAULT_FOLDER_NAME = 'Inbox';
let db;
// Cached prepared statements (initialized after DB open)
let stmts;
function initDatabase() {
    const dbPath = path.join(main_1.app.getPath('userData'), 'evnote.db');
    db = new better_sqlite3_1.default(dbPath);
    db.pragma('journal_mode = WAL');
    db.pragma('foreign_keys = ON');
    db.exec(`
    CREATE TABLE IF NOT EXISTS notes (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      title TEXT NOT NULL DEFAULT '',
      content TEXT NOT NULL DEFAULT '',
      tags TEXT NOT NULL DEFAULT '[]',
      created_at TEXT DEFAULT (datetime('now')),
      updated_at TEXT DEFAULT (datetime('now'))
    );

    CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
      title,
      content,
      content=notes,
      content_rowid=id
    );

    CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
      INSERT INTO notes_fts(rowid, title, content) VALUES (new.id, new.title, new.content);
    END;

    CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
      INSERT INTO notes_fts(notes_fts, rowid, title, content) VALUES('delete', old.id, old.title, old.content);
    END;

    CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
      INSERT INTO notes_fts(notes_fts, rowid, title, content) VALUES('delete', old.id, old.title, old.content);
      INSERT INTO notes_fts(rowid, title, content) VALUES (new.id, new.title, new.content);
    END;

    CREATE TABLE IF NOT EXISTS folders (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL,
      parent_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,
      is_default INTEGER NOT NULL DEFAULT 0,
      created_at TEXT DEFAULT (datetime('now'))
    );
  `);
    migrateNotesFolderColumn();
    // Cache prepared statements after schema is ready
    stmts = {
        getAll: db.prepare('SELECT * FROM notes ORDER BY updated_at DESC'),
        getById: db.prepare('SELECT * FROM notes WHERE id = ?'),
        create: db.prepare('INSERT INTO notes (title, content, tags, folder_id) VALUES (?, ?, ?, ?)'),
        delete: db.prepare('DELETE FROM notes WHERE id = ?'),
        search: db.prepare(`
      SELECT notes.* FROM notes
      JOIN notes_fts ON notes.id = notes_fts.rowid
      WHERE notes_fts MATCH ?
      ORDER BY rank
    `),
        getByTag: db.prepare("SELECT * FROM notes WHERE tags LIKE ? ESCAPE '\\' ORDER BY updated_at DESC"),
        getAllTags: db.prepare('SELECT tags FROM notes'),
        getFolders: db.prepare('SELECT * FROM folders ORDER BY id'),
        getFolderById: db.prepare('SELECT * FROM folders WHERE id = ?'),
        getDefaultFolder: db.prepare('SELECT * FROM folders WHERE is_default = 1 LIMIT 1'),
        // The folder itself and everything below it (the UNION also protects against a parent cycle)
        subtreeIds: db.prepare(`
      WITH RECURSIVE subtree(id) AS (
        SELECT id FROM folders WHERE id = ?
        UNION
        SELECT f.id FROM folders f JOIN subtree s ON f.parent_id = s.id
      )
      SELECT id FROM subtree
    `),
    };
    ensureDefaultFolder();
}
/** `ALTER TABLE ... ADD COLUMN` is not idempotent, hence the check. */
function addColumnIfMissing(table, column, definition) {
    const columns = db.pragma(`table_info(${table})`);
    if (!columns.some(existing => existing.name === column)) {
        db.exec(`ALTER TABLE ${table} ADD COLUMN ${column} ${definition}`);
    }
}
/**
 * Adds the folder-related columns to databases created before the folders existed. Existing notes get a NULL
 * `folder_id` ("at the root"); a NULL `sort_order` means "never positioned by the user" (alphabetical order).
 */
function migrateNotesFolderColumn() {
    addColumnIfMissing('notes', 'folder_id', 'INTEGER REFERENCES folders(id) ON DELETE SET NULL');
    addColumnIfMissing('notes', 'sort_order', 'INTEGER');
    addColumnIfMissing('folders', 'sort_order', 'INTEGER');
    db.exec('CREATE INDEX IF NOT EXISTS idx_notes_folder_id ON notes(folder_id)');
}
/** The default folder must always exist: the notes created on the fly land in it. */
function ensureDefaultFolder() {
    if (!stmts.getDefaultFolder.get()) {
        db.prepare('INSERT INTO folders (name, parent_id, is_default) VALUES (?, NULL, 1)').run(DEFAULT_FOLDER_NAME);
    }
}
function closeDatabase() {
    if (db) {
        db.close();
    }
}
/** Escapes special FTS5 characters so user input is treated as literal text. */
function sanitizeFtsQuery(query) {
    return query
        .trim()
        .split(/\s+/)
        .filter(token => token.length > 0)
        .map(token => `"${token.replace(/"/g, '""')}"`)
        .join(' ');
}
/** Escapes LIKE wildcards so user input is treated as literal text. */
function escapeLike(value) {
    return value
        .replace(/\\/g, '\\\\')
        .replace(/%/g, '\\%')
        .replace(/_/g, '\\_');
}
exports.noteOps = {
    getAll() {
        return stmts.getAll.all();
    },
    getById(id) {
        return stmts.getById.get(id);
    },
    create(title, content, tags = [], folderId = null) {
        // A folder that does not exist (anymore) must not make the note creation fail
        const validFolderId = folderId !== null && exports.folderOps.getById(folderId) ? folderId : null;
        const result = stmts.create.run(title, content, JSON.stringify(tags), validFolderId);
        return this.getById(result.lastInsertRowid);
    },
    update(id, data) {
        const fields = [];
        const values = [];
        if (data.title !== undefined) {
            fields.push('title = ?');
            values.push(data.title);
        }
        if (data.content !== undefined) {
            fields.push('content = ?');
            values.push(data.content);
        }
        if (data.tags !== undefined) {
            fields.push('tags = ?');
            values.push(data.tags);
        }
        if (fields.length === 0)
            return this.getById(id);
        fields.push("updated_at = datetime('now')");
        values.push(id);
        db.prepare(`UPDATE notes SET ${fields.join(', ')} WHERE id = ?`).run(...values);
        return this.getById(id);
    },
    /** Moves the note without touching `updated_at`: a reorganization is not an edit. */
    moveToFolder(id, folderId) {
        if (folderId !== null && !exports.folderOps.getById(folderId)) {
            return this.getById(id);
        }
        // A note arriving in another folder loses its position there: it is listed after the positioned ones
        db.prepare('UPDATE notes SET folder_id = ?, sort_order = CASE WHEN folder_id IS ? THEN sort_order ELSE NULL END WHERE id = ?')
            .run(folderId, folderId, id);
        return this.getById(id);
    },
    /**
     * Puts the note in `folderId` at the position given by `orderedIds` (the ids of the whole folder content,
     * in display order). Only the notes really in that folder are renumbered.
     */
    place(id, folderId, orderedIds) {
        const moved = this.moveToFolder(id, folderId);
        if (!moved || moved.folder_id !== folderId) {
            return moved;
        }
        const setOrder = db.prepare('UPDATE notes SET sort_order = ? WHERE id = ? AND folder_id IS ?');
        db.transaction(() => {
            orderedIds.forEach((noteId, index) => setOrder.run(index + 1, noteId, folderId));
        })();
        return this.getById(id);
    },
    delete(id) {
        stmts.delete.run(id);
    },
    search(query) {
        if (!query.trim())
            return this.getAll();
        const sanitized = sanitizeFtsQuery(query);
        if (!sanitized)
            return this.getAll();
        return stmts.search.all(sanitized + '*');
    },
    getByTag(tag) {
        const escaped = escapeLike(tag.replace(/"/g, ''));
        return stmts.getByTag.all(`%"${escaped}"%`);
    },
    getAllTags() {
        const rows = stmts.getAllTags.all();
        const tagSet = new Set();
        for (const row of rows) {
            try {
                const tags = JSON.parse(row.tags);
                tags.forEach(t => tagSet.add(t));
            }
            catch { /* skip malformed tags */ }
        }
        return Array.from(tagSet).sort();
    }
};
/** Trims the name and rejects what cannot be a folder name; `null` when invalid. */
function normalizeFolderName(name) {
    if (typeof name !== 'string') {
        return null;
    }
    const trimmed = name.trim();
    return trimmed.length > 0 && trimmed.length <= exports.MAX_FOLDER_NAME_LENGTH ? trimmed : null;
}
function subtreeIds(folderId) {
    return stmts.subtreeIds.all(folderId).map(row => row.id);
}
exports.folderOps = {
    getAll() {
        return stmts.getFolders.all();
    },
    getById(id) {
        return stmts.getFolderById.get(id);
    },
    create(name, parentId) {
        const normalized = normalizeFolderName(name);
        if (normalized === null || (parentId !== null && !this.getById(parentId))) {
            return undefined;
        }
        const result = db.prepare('INSERT INTO folders (name, parent_id) VALUES (?, ?)').run(normalized, parentId);
        return this.getById(result.lastInsertRowid);
    },
    rename(id, name) {
        const normalized = normalizeFolderName(name);
        if (normalized === null || !this.getById(id)) {
            return undefined;
        }
        db.prepare('UPDATE folders SET name = ? WHERE id = ?').run(normalized, id);
        return this.getById(id);
    },
    /** Refuses (returns `undefined`) to move the default folder, or a folder into itself or its own subtree. */
    move(id, parentId) {
        const folder = this.getById(id);
        if (!folder || folder.is_default === 1 || (parentId !== null && !this.getById(parentId))) {
            return undefined;
        }
        if (parentId !== null && subtreeIds(id).includes(parentId)) {
            return undefined;
        }
        db.prepare('UPDATE folders SET parent_id = ?, sort_order = CASE WHEN parent_id IS ? THEN sort_order ELSE NULL END WHERE id = ?')
            .run(parentId, parentId, id);
        return this.getById(id);
    },
    /**
     * Puts the folder under `parentId` at the position given by `orderedIds` (the ids of all the children of
     * that parent, in display order). Refused (`undefined`) when the move itself is, see `move`.
     */
    place(id, parentId, orderedIds) {
        const folder = this.getById(id);
        if (!folder) {
            return undefined;
        }
        if (folder.parent_id !== parentId && !this.move(id, parentId)) {
            return undefined;
        }
        const setOrder = db.prepare('UPDATE folders SET sort_order = ? WHERE id = ? AND parent_id IS ?');
        db.transaction(() => {
            orderedIds.forEach((folderId, index) => setOrder.run(index + 1, folderId, parentId));
        })();
        return this.getById(id);
    },
    /**
     * Deletes the folder and its sub-folders. The notes inside are either moved to the root or deleted with them.
     * Everything happens in one transaction so a failure cannot leave notes pointing to deleted folders.
     * Returns the ids of the deleted notes, or `null` when refused (default folder, unknown folder).
     */
    delete(id, mode) {
        const folder = this.getById(id);
        if (!folder || folder.is_default === 1) {
            return null;
        }
        const ids = subtreeIds(id);
        const marks = ids.map(() => '?').join(', ');
        return db.transaction(() => {
            let deletedNoteIds = [];
            if (mode === 'deleteAll') {
                const rows = db.prepare(`SELECT id FROM notes WHERE folder_id IN (${marks})`).all(...ids);
                deletedNoteIds = rows.map(row => row.id);
                db.prepare(`DELETE FROM notes WHERE folder_id IN (${marks})`).run(...ids);
            }
            else {
                db.prepare(`UPDATE notes SET folder_id = NULL WHERE folder_id IN (${marks})`).run(...ids);
            }
            // Sub-folders go away through ON DELETE CASCADE
            db.prepare('DELETE FROM folders WHERE id = ?').run(id);
            return deletedNoteIds;
        })();
    },
};
