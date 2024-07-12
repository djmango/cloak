import os
import uuid
import asyncio
import asyncpg
from datetime import datetime
from typing import List, Optional
import re
import logging

from dotenv import load_dotenv

load_dotenv()

# Setup logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

# Constants
USER_INFO_REGEX = re.compile(r'(?s)<user information>(.*?)</user information>')

class Memory:
    def __init__(self, id: uuid.UUID, user_id: str, content: str, created_at: datetime,
                 updated_at: datetime, deleted_at: Optional[datetime] = None,
                 grouping: Optional[str] = None):
        self.id = id
        self.user_id = user_id
        self.content = content
        self.created_at = created_at
        self.updated_at = updated_at
        self.deleted_at = deleted_at
        self.grouping = grouping

async def get_all_users():
    load_dotenv()
    database_url = os.getenv('DATABASE_URL')
    
    if not database_url:
        raise ValueError("DATABASE_URL not found in .env file")

    async with asyncpg.create_pool(database_url) as pool:
        async with pool.acquire() as conn:
            query = "SELECT id, first_name, last_name, email, created_at, updated_at FROM users"
            rows = await conn.fetch(query)
            
            users = [
                {
                    'id': row['id'],
                    'first_name': row['first_name'],
                    'last_name': row['last_name'],
                    'email': row['email'],
                    'created_at': row['created_at'],
                    'updated_at': row['updated_at']
                }
                for row in rows
            ]
            
            return users
        
async def get_all_memories(user_id: str) -> List[Memory]:
    load_dotenv()
    database_url = os.getenv('DATABASE_URL')
    
    if not database_url:
        raise ValueError("DATABASE_URL not found in .env file")

    async with asyncpg.create_pool(database_url) as pool:
        start = datetime.now()

        query = """
        SELECT id, user_id, content, created_at, updated_at, deleted_at, grouping
        FROM memories 
        WHERE user_id = $1 AND deleted_at IS NULL
        """
        rows = await pool.fetch(query, user_id)

        result = []
        for row in rows:
            memory = Memory(
                id=row['id'],
                user_id=row['user_id'],
                content=row['content'],
                created_at=row['created_at'],
                updated_at=row['updated_at'],
                deleted_at=row['deleted_at'],
                grouping=row['grouping']
            )
            result.append(memory)

        duration = datetime.now() - start
        logger.info(f"Query execution time: {duration}")
        logger.info(f"All memories found: {len(result)}")
        return result

def format_memories(memories: List[Memory]) -> str:
    formatted_memories = []

    for index, memory in enumerate(memories):
        logger.info(f"Memory {index}: Content: {memory.content}")
        matches = USER_INFO_REGEX.findall(memory.content)
        if matches:
            logger.info(f"Memory {index}: Regex match found")
            extracted = matches[0].strip()
            logger.info(f"Memory {index}: Extracted content length: {len(extracted)} chars")
            formatted_memories.append(extracted)
        else:
            logger.info(f"Memory {index}: No regex match found")

    return '\n'.join(formatted_memories)

async def get_all_user_memories(pool: asyncpg.Pool, user_id: str) -> str:
    # Fetch all memories for the user
    user_memories = await get_all_memories(pool, user_id)
    formatted_memories = format_memories(user_memories)

    # Save formatted memories to file
    save_dir = os.path.join('logs', 'user_memories')
    os.makedirs(save_dir, exist_ok=True)
    file_path = os.path.join(save_dir, f"{user_id}.txt")
    with open(file_path, 'w') as f:
        f.write(formatted_memories)

    return formatted_memories
# Example usage
async def main():
    load_dotenv()
    database_url = os.getenv('DATABASE_URL')
    
    if not database_url:
        raise ValueError("DATABASE_URL not found in .env file")

    async with asyncpg.create_pool(database_url) as pool:
        # Get all users
        users = await get_all_users()
        # Process users in chunks of 500
        chunk_size = 500
        for i in range(0, len(users), chunk_size):
            user_chunk = users[i:i+chunk_size]
            
            # Create tasks for each user in the chunk
            tasks = [get_all_user_memories(pool, user['id']) for user in user_chunk]
            
            # Run tasks concurrently
            results = await asyncio.gather(*tasks)
            
            for user, formatted_memories in zip(user_chunk, results):
                print(f"Formatted memories saved for user {user['id']}")

    # Pool is automatically closed when exiting the context manager

if __name__ == "__main__":
    asyncio.run(main())