#!/usr/bin/env python3
import sqlite3
import sys
import os
import random
import struct
from array import array


def create_db(path, n_points=15000, dim=768, batch=500):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    if os.path.exists(path):
        print(f"Removing existing DB at {path}")
        os.remove(path)

    conn = sqlite3.connect(path)
    cur = conn.cursor()

    # Create table
    cur.execute('''
        CREATE TABLE summaries (
            identifier INTEGER PRIMARY KEY,
            original_source_link TEXT,
            summary TEXT,
            model TEXT,
            embedding_model TEXT,
            timestamped_summary_in_youtube_format TEXT,
            embedding BLOB
        )
    ''')
    conn.commit()

    insert_sql = '''
        INSERT INTO summaries (
            identifier, original_source_link, summary, model, embedding_model, timestamped_summary_in_youtube_format, embedding
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
    '''

    print(f"Generating {n_points} rows with dim={dim} into {path}...")

    id_counter = 1
    rows = []
    for i in range(n_points):
        # deterministic-ish content
        original_source_link = f"https://example.com/item/{id_counter}"
        summary = f"Synthetic summary {id_counter}"
        model = "synthetic-model"
        embedding_model = "synthetic-emb-model"
        timestamped_summary = f"00:00:00 {id_counter}"

        # generate random floats in range [0,1)
        arr = array('f', (random.random() for _ in range(dim)))
        blob = arr.tobytes()

        rows.append((id_counter, original_source_link, summary, model, embedding_model, timestamped_summary, sqlite3.Binary(blob)))
        id_counter += 1

        # batch insert
        if len(rows) >= batch:
            cur.executemany(insert_sql, rows)
            conn.commit()
            print(f"Inserted {id_counter-1} rows...")
            rows = []

    if rows:
        cur.executemany(insert_sql, rows)
        conn.commit()
        print(f"Inserted {id_counter-1} rows (final) ...")

    conn.close()
    print("Database creation complete.")


if __name__ == '__main__':
    path = sys.argv[1] if len(sys.argv) > 1 else 'data/synthetic_15000.db'
    n_points = int(sys.argv[2]) if len(sys.argv) > 2 else 15000
    dim = int(sys.argv[3]) if len(sys.argv) > 3 else 768
    create_db(path, n_points=n_points, dim=dim)
