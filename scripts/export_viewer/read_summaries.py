import sqlite3
import pandas as pd
import os

# Find the database in the current working directory, defaulting to "summaries_compact.db"
db_path = os.environ.get("COMPACT_DB_PATH", "summaries_compact.db")

if not os.path.exists(db_path):
    print(f"Error: Database file '{db_path}' not found.")
    print("Please place the decompressed 'summaries_compact.db' in this directory or set the COMPACT_DB_PATH environment variable.")
    exit(1)

# Establish a connection to the SQLite database
conn = sqlite3.connect(db_path)

# Query the summaries table to load rows
query = "SELECT identifier, original_source_link, model, cost, summary FROM summaries"
df = pd.read_sql_query(query, conn)

# Close the database connection
conn.close()

# Print status and display the first few rows
print(f"Successfully loaded {len(df)} summaries using pandas!\n")
print(df.head())
