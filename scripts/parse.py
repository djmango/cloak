import os
import re

def read_file(file_path):
    with open(file_path, 'r') as file:
        return file.read()

def extract_user_information(text):
    pattern = r'<user information>(.*?)</user information>'
    matches = re.findall(pattern, text, re.DOTALL)
    if matches:
        return [match.strip() for match in matches]
    else:
        return ["No user information found."]

def parse_user_information(user_info):
    return [item.strip() for item in user_info.split('•') if item.strip()]

def save_user_information(file_path, user_info):
    cwd = os.path.dirname(os.path.realpath(__file__))
    base_name = os.path.basename(file_path)
    new_file_name = os.path.splitext(base_name)[0] + '.txt'
    output_path = os.path.join(cwd, 'logs', new_file_name)
    
    with open(output_path, 'w') as file:
        for section in user_info:
            file.write("Memory:\n")
            for item in section:
                file.write(f"• {item}\n")
            file.write("\n")  # Add a blank line between sections

# Read the file
file_path = '/Users/minjunes/cloak/scripts/logs/4bf7c374-1390-4887-9e5f-bfad6e75bd04-user_01HY5EW9Z5XVE34GZXKH4NC2Y1.csv'
file_content = read_file(file_path)

# Extract all user information sections
user_info_sections = extract_user_information(file_content)

# Parse all user information
parsed_user_info = [parse_user_information(section) for section in user_info_sections]

# Save the parsed user information to a new file
save_user_information(file_path, parsed_user_info)
