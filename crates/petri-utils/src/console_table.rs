use std::fmt::Display;

/// A builder which can be used to create a printable table.
#[derive(Clone, Default)]
pub struct Builder {
    columns: Vec<Column>,
}

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
struct Column {
    options: ColumnOptions,
    max_width: usize,
    rows: Vec<String>,
}

impl Builder {
    /// Constructs a new `Builder`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a set of columns with the specified options, and invokes a
    /// builder closure to fill data.
    pub fn with_new_columns<C: ColumnCollection, B>(mut self, columns: C, builder: B) -> Self
    where
        B: FnOnce(&mut dyn FnMut(<C as ColumnCollection>::RowInserterArg)),
    {
        columns.build(&mut self.columns, builder);
        self
    }

    /// Builds the table and returns it as a `String` value.
    pub fn build(&self) -> String {
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

impl Display for Builder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.build())
    }
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

#[allow(private_interfaces)]
pub trait ColumnCollection {
    type RowInserterArg;

    fn build<B>(self, columns: &mut Vec<Column>, builder: B)
    where
        B: FnOnce(&mut dyn FnMut(Self::RowInserterArg));
}

macro_rules! impl_column_collection {
    ($($label:tt),*) => {
        macro_rules! replace_with {
            ($t:tt => $r:tt) => { $r };
        }

        #[allow(private_interfaces)]
        impl ColumnCollection for ($(replace_with!($label => ColumnOptions),)*) {
            type RowInserterArg = ($(replace_with!($label => String),)*);

            fn build<B>(self, columns: &mut Vec<Column>, builder: B)
            where
                B: FnOnce(&mut dyn FnMut(Self::RowInserterArg))
            {
                paste::paste! {
                    $(
                        let title_width = self.$label.title.len();
                        let mut [<column_ $label>] = Column {
                            options: self.$label,
                            max_width: title_width,
                            rows: vec![],
                        };
                    )*
                    let mut inserter = |($([<field_ $label>],)*): ($(replace_with!($label => String),)*)| {
                        $(
                            [<column_ $label>].max_width = [<column_ $label>].max_width.max([<field_ $label>].len());
                            [<column_ $label>].rows.push([<field_ $label>]);
                        )*
                    };
                    builder(&mut inserter);
                    $(
                        columns.push([<column_ $label>]);
                    )*
                }
            }
        }
    };
}

impl_column_collection!(0);
impl_column_collection!(0, 1);
impl_column_collection!(0, 1, 2);
impl_column_collection!(0, 1, 2, 3);
impl_column_collection!(0, 1, 2, 3, 4);
impl_column_collection!(0, 1, 2, 3, 4, 5);
impl_column_collection!(0, 1, 2, 3, 4, 5, 6);
impl_column_collection!(0, 1, 2, 3, 4, 5, 6, 7);

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::{Alignment, Builder, ColumnOptions};

    #[test]
    fn test_simple_table() {
        let key_column = ColumnOptions::new("KEY")
            .alignment(Alignment::Right)
            .spacing(2);
        let value_column = ColumnOptions::new("VALUE").spacing(3);
        let note_column = ColumnOptions::new("NOTE");

        let table = Builder::new()
            .with_new_columns((key_column, value_column, note_column), |insert| {
                insert((
                    "first".to_string(),
                    "a".to_string(),
                    "This is a note".to_string(),
                ));
                insert((
                    "second".to_string(),
                    "b".to_string(),
                    "This is another note".to_string(),
                ));
            })
            .build();

        assert_eq!(
            table,
            indoc! {"
                   KEY  VALUE   NOTE
                 first  a       This is a note
                second  b       This is another note"}
        )
    }
}
