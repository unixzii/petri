use std::fmt::Display;

/// Options for customizing a table column.
#[derive(Clone, Debug)]
pub struct ColumnOptions {
    title: String,
    alignment: Alignment,
    spacing: u32,
}

/// Possible alignments used to align text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Alignment {
    Left,
    Right,
}

#[derive(Clone)]
struct CoreBuilder {
    columns: Vec<Column>,
}

#[derive(Clone)]
struct Column {
    options: ColumnOptions,
    max_width: usize,
    rows: Vec<String>,
}

impl ColumnOptions {
    /// Constructs a new `ColumnOptions` with the specified title.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            alignment: Alignment::Left,
            spacing: 1,
        }
    }

    /// Sets the alignment for the column.
    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Sets the trailing spacing for the column.
    pub fn spacing(mut self, spacing: u32) -> Self {
        self.spacing = spacing;
        self
    }
}

impl CoreBuilder {
    #[inline]
    fn new(columns: Vec<Column>) -> Self {
        Self { columns }
    }

    fn push_row(&mut self, column_idx: usize, field: String) {
        let column = &mut self.columns[column_idx];
        column.max_width = column.max_width.max(field.len());
        column.rows.push(field);
    }

    fn build(&self) -> String {
        let mut table = String::new();
        if self.columns.is_empty() {
            return table;
        }

        let last_column_idx = self.columns.len() - 1;
        let row_count = self.columns[0].rows.len();
        for pseudo_row_idx in 0..(row_count + 1) {
            for (idx, column) in self.columns.iter().enumerate() {
                if column.options.alignment == Alignment::Left {
                    table.push_str(&format!(
                        "{:<width$}",
                        if pseudo_row_idx == 0 {
                            &column.options.title
                        } else {
                            &column.rows[pseudo_row_idx - 1]
                        },
                        width = if idx != last_column_idx {
                            column.max_width
                        } else {
                            0
                        }
                    ));
                } else {
                    table.push_str(&format!(
                        "{:>width$}",
                        if pseudo_row_idx == 0 {
                            &column.options.title
                        } else {
                            &column.rows[pseudo_row_idx - 1]
                        },
                        width = if idx != last_column_idx {
                            column.max_width
                        } else {
                            0
                        }
                    ));
                }
                if idx != last_column_idx {
                    table.push_str(&" ".repeat(column.options.spacing as usize));
                }
            }

            if pseudo_row_idx != row_count {
                table.push('\n');
            }
        }

        table
    }
}

/// A collection of column definitions, which can be used to make
/// builders for creating tables with those columns.
pub trait ColumnCollection {
    /// The type of the table builder being created.
    ///
    /// This can be different according to the number of columns in
    /// the collection, ensuring the row operations are type-safe.
    type Builder;

    /// Creates a table builder from the collection.
    fn into_table_builder(self) -> Self::Builder;
}

mod __private {
    #[macro_export]
    macro_rules! replace_with {
        ($t:tt => $r:tt) => {
            $r
        };
    }

    pub use replace_with;
}

macro_rules! impl_column_collection {
    ($builder:ident, $($label:tt),*) => {
        #[derive(Clone)]
        pub struct $builder {
            core: CoreBuilder,
        }

        paste::paste! {
            impl $builder {
                /// Appends a row with the specified field values.
                pub fn push_row(&mut self, $([<field_ $label>]: String),*) {
                    $(
                        self.core.push_row($label, [<field_ $label>]);
                    )*
                }

                /// Builds the table and returns it as a `String` value.
                pub fn build(&self) -> String {
                    self.core.build()
                }
            }
        }

        impl Display for $builder {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.build())
            }
        }

        impl ColumnCollection for ($(__private::replace_with!($label => ColumnOptions),)*) {
            type Builder = $builder;

            fn into_table_builder(self) -> $builder {
                paste::paste! {
                    let core = CoreBuilder::new(
                        vec![
                            $({
                                let title_width = self.$label.title.len();
                                Column {
                                    options: self.$label,
                                    max_width: title_width,
                                    rows: vec![],
                                }
                            },)*
                        ]
                    );
                    $builder { core }
                }
            }
        }
    };
}

impl_column_collection!(Builder1, 0);
impl_column_collection!(Builder2, 0, 1);
impl_column_collection!(Builder3, 0, 1, 2);
impl_column_collection!(Builder4, 0, 1, 2, 3);
impl_column_collection!(Builder5, 0, 1, 2, 3, 4);
impl_column_collection!(Builder6, 0, 1, 2, 3, 4, 5);
impl_column_collection!(Builder7, 0, 1, 2, 3, 4, 5, 6);
impl_column_collection!(Builder8, 0, 1, 2, 3, 4, 5, 6, 7);

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::{Alignment, ColumnCollection, ColumnOptions};

    #[test]
    fn test_simple_table() {
        let key_column = ColumnOptions::new("KEY")
            .alignment(Alignment::Right)
            .spacing(2);
        let value_column = ColumnOptions::new("VALUE").spacing(3);
        let note_column = ColumnOptions::new("NOTE");

        let mut builder = (key_column, value_column, note_column).into_table_builder();
        builder.push_row(
            "first".to_string(),
            "a".to_string(),
            "This is a note".to_string(),
        );
        builder.push_row(
            "second".to_string(),
            "b".to_string(),
            "This is another note".to_string(),
        );

        let table = builder.build();
        assert_eq!(
            table,
            indoc! {"
                   KEY  VALUE   NOTE
                 first  a       This is a note
                second  b       This is another note"}
        )
    }
}
