#!/usr/bin/env bash
set -euo pipefail

# Couleurs pour l'affichage
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # Pas de couleur

cargo fmt --all
echo -e "${BLUE}=== Lancement des vérifications locales (Pipeline Rapide) ===${NC}"

# 1. Vérification du formatage
echo -e "\n${YELLOW}[1/5] Vérification du formatage (cargo fmt)...${NC}"
if ! cargo fmt --all -- --check; then
	echo -e "${RED}✘ Erreur de formatage détectée ! Lancez 'cargo fmt --all' pour corriger.${NC}"
	exit 1
fi
echo -e "${GREEN}✔ Formatage correct.${NC}"

# 2. Analyse statique (Clippy)
echo -e "\n${YELLOW}[2/5] Analyse statique (cargo clippy)...${NC}"
if ! cargo clippy --all-targets -- -D warnings; then
	echo -e "${RED}✘ Clippy a trouvé des avertissements ou erreurs !${NC}"
	exit 1
fi
echo -e "${GREEN}✔ L'analyse statique a réussi.${NC}"

# 3. Exécution des tests unitaires
echo -e "\n${YELLOW}[3/5] Exécution des tests (cargo test)...${NC}"
if ! cargo test; then
	echo -e "${RED}✘ Des tests ont échoué !${NC}"
	exit 1
fi
echo -e "${GREEN}✔ Tous les tests sont passés.${NC}"

# 4. Compilation rapide (Debug)
echo -e "\n${YELLOW}[4/5] Compilation en mode Debug...${NC}"
if ! cargo build; then
	echo -e "${RED}✘ Échec de la compilation !${NC}"
	exit 1
fi
echo -e "${GREEN}✔ Compilation terminée avec succès.${NC}"

# 5. Build du Frontend (npm run build)
echo -e "\n${YELLOW}[5/5] Vérification du Frontend (npm run build)...${NC}"
if ! npm run build; then
	echo -e "${RED}✘ Échec du build du Frontend !${NC}"
	exit 1
fi
echo -e "${GREEN}✔ Build du Frontend terminé avec succès.${NC}"

echo -e "\n${GREEN}===========================================${NC}"
echo -e "${GREEN}  ✓ TOUTES LES VÉRIFICATIONS SONT OK !      ${NC}"
echo -e "${GREEN}===========================================${NC}"
