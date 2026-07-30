import os
import re

directory = '.'

for root, _, files in os.walk(directory):
    for file in files:
        if file.endswith('.rs'):
            filepath = os.path.join(root, file)
            with open(filepath, 'r') as f:
                content = f.read()

            # Find instances of SLAConfig { ... reward_base: <val> }
            # and append the new fields before the closing brace.
            # We'll use a regex that captures everything up to the last field (reward_base)
            # and inserts the missing fields.
            
            # The pattern looks for `SLAConfig {` followed by anything, then `reward_base: <val>`
            # optionally followed by a comma and whitespace, then `}`
            pattern = re.compile(r'(SLAConfig\s*\{[^}]*reward_base:\s*[^,}]+)(,?\s*\})')
            
            def repl(match):
                prefix = match.group(1)
                suffix = match.group(2)
                # If it already has top_tier_multiplier, don't replace
                if 'top_tier_multiplier' in prefix:
                    return match.group(0)
                
                # Determine indentation or simple formatting
                # If there are newlines in the prefix, add newlines, otherwise just spaces
                if '\n' in prefix:
                    return prefix + ',\n            top_tier_multiplier: 200,\n            excel_tier_multiplier: 150,\n            good_tier_multiplier: 100' + suffix
                else:
                    return prefix + ', top_tier_multiplier: 200, excel_tier_multiplier: 150, good_tier_multiplier: 100' + suffix

            new_content = pattern.sub(repl, content)

            if new_content != content:
                with open(filepath, 'w') as f:
                    f.write(new_content)
                print(f"Updated {filepath}")

