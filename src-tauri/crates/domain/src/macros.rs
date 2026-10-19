/// Déclare une énumération sans données dont chaque variante a une forme textuelle stable
/// (celle des fichiers et de l'IPC), avec `ALL`, `as_str` et `parse`.
macro_rules! text_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident => $text:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$variant_meta])* $variant ),+
        }

        impl $name {
            /// Toutes les variantes, dans l'ordre de déclaration.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// Forme textuelle stable de la variante.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }

            /// Variante correspondant exactement à `text`, `None` sinon.
            pub fn parse(text: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|variant| variant.as_str() == text)
            }
        }
    };
}
