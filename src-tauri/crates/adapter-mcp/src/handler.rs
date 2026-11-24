//! Le serveur MCP vu par le protocole : la liste des outils et leur exécution.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

use crate::server::ChangeListener;
use crate::tools::{
    self, AppendToNoteArgs, CreateNoteArgs, EditNoteArgs, GetNoteArgs, GetNotesArgs, ListNotesArgs,
    McpServices, McpServicesSource, SearchNotesArgs, ToolResult, UpdateNoteArgs,
};

/// Réponse à une IA tant que le profil actif attend son mot de passe.
const LOCKED: &str = "Daftr is locked: the user must unlock the active profile in the app.";

/// Consignes données aux IA à la connexion.
const INSTRUCTIONS: &str = "Daftr is the user's markdown note-taking app. Notes live in \
folders (the default one is Inbox) and carry tags; the first line of a note is its title. \
Use search_notes or list_notes to find notes, get_note (or get_notes) to read them, and \
create_note, edit_note, append_to_note or update_note to write. Prefer edit_note to change \
part of a note and get_note with a section or lines to read part of one: they save tokens. \
Notes cannot be deleted from here. Dates are UTC.";

/// Une connexion d'IA : chaque session MCP a la sienne, toutes partagent les services.
#[derive(Clone)]
pub(crate) struct DaftrTools {
    services: Arc<dyn McpServicesSource>,
    on_change: ChangeListener,
    tool_router: ToolRouter<Self>,
}

impl DaftrTools {
    /// Outils branchés sur les services ; `on_change` est appelé après chaque écriture.
    pub(crate) fn new(services: Arc<dyn McpServicesSource>, on_change: ChangeListener) -> Self {
        Self {
            services,
            on_change,
            tool_router: Self::tool_router(),
        }
    }

    /// Lit les notes hors des fils de tokio (les services sont synchrones).
    async fn read(
        &self,
        tool: impl FnOnce(&McpServices) -> ToolResult + Send + 'static,
    ) -> Result<String, String> {
        let services = self.services.current().ok_or_else(|| LOCKED.to_owned())?;
        let value = tokio::task::spawn_blocking(move || tool(&services))
            .await
            .map_err(|error| format!("Internal error: {error}"))??;

        serde_json::to_string(&value).map_err(|error| format!("Internal error: {error}"))
    }

    /// Comme [`Self::read`], puis prévient l'application que les notes ont changé.
    async fn write(
        &self,
        tool: impl FnOnce(&McpServices) -> ToolResult + Send + 'static,
    ) -> Result<String, String> {
        let written = self.read(tool).await?;
        (self.on_change)();
        Ok(written)
    }
}

#[tool_router]
impl DaftrTools {
    #[tool(
        description = "List every folder of notes: id, name, parent_id (null at the root), path from the root and whether it is the default folder (Inbox).",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_folders(&self) -> Result<String, String> {
        self.read(tools::list_folders).await
    }

    #[tool(
        description = "List notes without their content (id, title, folder_id, tags, updated_at), most recently updated first. Optionally only those of a folder or carrying a tag. 'total' counts every matching note.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_notes(
        &self,
        Parameters(args): Parameters<ListNotesArgs>,
    ) -> Result<String, String> {
        self.read(move |services| tools::list_notes(services, args))
            .await
    }

    #[tool(
        description = "Full-text search in the titles and contents of the notes. Returns the matching notes, best first, with a snippet around the match (no full content: use get_note).",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn search_notes(
        &self,
        Parameters(args): Parameters<SearchNotesArgs>,
    ) -> Result<String, String> {
        self.read(move |services| tools::search_notes(services, args))
            .await
    }

    #[tool(
        description = "Read a note: its metadata and its markdown content, whole by default. For a long note, read only a section (by heading) or a range of lines; 'lines' and 'total_lines' then tell which part came back.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_note(&self, Parameters(args): Parameters<GetNoteArgs>) -> Result<String, String> {
        self.read(move |services| tools::get_note(services, args))
            .await
    }

    #[tool(
        description = "Read several notes (at most 20) in one call, with their full content. Ids that do not exist are listed in 'missing'.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_notes(
        &self,
        Parameters(args): Parameters<GetNotesArgs>,
    ) -> Result<String, String> {
        self.read(move |services| tools::get_notes(services, args))
            .await
    }

    #[tool(
        description = "List every tag used by the notes, sorted.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_tags(&self) -> Result<String, String> {
        self.read(tools::list_tags).await
    }

    #[tool(
        description = "Create a note from markdown content. It goes to the Inbox folder unless folder_id is given. Returns the new note without its content.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn create_note(
        &self,
        Parameters(args): Parameters<CreateNoteArgs>,
    ) -> Result<String, String> {
        self.write(move |services| tools::create_note(services, args))
            .await
    }

    #[tool(
        description = "Replace the whole markdown content of a note; its first line becomes its title. To change only part of a note, use edit_note instead.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_note(
        &self,
        Parameters(args): Parameters<UpdateNoteArgs>,
    ) -> Result<String, String> {
        self.write(move |services| tools::update_note(services, args))
            .await
    }

    #[tool(
        description = "Replace an exact passage of a note (old_text -> new_text), keeping the rest untouched: no need to send the whole note. Fails if old_text is missing or appears several times, unless replace_all is set. Returns the note without its content.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn edit_note(
        &self,
        Parameters(args): Parameters<EditNoteArgs>,
    ) -> Result<String, String> {
        self.write(move |services| tools::edit_note(services, args))
            .await
    }

    #[tool(
        description = "Append markdown text at the end of a note, after a blank line, keeping the rest of the note.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn append_to_note(
        &self,
        Parameters(args): Parameters<AppendToNoteArgs>,
    ) -> Result<String, String> {
        self.write(move |services| tools::append_to_note(services, args))
            .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DaftrTools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("daftr", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}
