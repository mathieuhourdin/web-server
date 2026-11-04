CREATE TABLE writing_templates (
    id uuid PRIMARY KEY DEFAULT uuid_generate_v4(),
    slug text NOT NULL UNIQUE CHECK (slug ~ '^[a-z][a-z0-9_-]{0,79}$'),
    title text NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 200),
    description text NOT NULL DEFAULT '' CHECK (char_length(description)<=2000),
    content text NOT NULL CHECK (char_length(btrim(content)) BETWEEN 1 AND 20000),
    locale text NOT NULL DEFAULT 'fr' CHECK (char_length(locale)<=35 AND locale ~ '^[a-z]{2,3}(-[a-z0-9]{2,8})*$'),
    sort_order integer NOT NULL DEFAULT 0,
    is_active boolean NOT NULL DEFAULT TRUE,
    created_at timestamp NOT NULL DEFAULT NOW(),
    updated_at timestamp NOT NULL DEFAULT NOW()
);
CREATE INDEX writing_templates_catalogue_idx ON writing_templates (is_active,locale,sort_order,slug);
ALTER TABLE traces ADD COLUMN writing_template_id uuid REFERENCES writing_templates(id) ON DELETE RESTRICT;
CREATE INDEX traces_writing_template_id_idx ON traces (writing_template_id) WHERE writing_template_id IS NOT NULL;

INSERT INTO writing_templates(slug,title,description,content,sort_order) VALUES
('last_night_dream',$$Mon rêve de cette nuit$$,$$Ce dont vous avez rêvé, avant que ça s’efface.$$,$$Ce dont je me souviens :

Où ça se passait :
Qui était là :
Ce que je ressentais :
Ce qui me reste au réveil :$$,10),
('daily_scene',$$La scène du jour$$,$$Un moment précis de la journée que vous voulez garder.$$,$$Un moment précis que je veux garder :

Où j’étais :
Avec qui :
Ce qui s’est passé :
Ce que j’ai envie d’en retenir :$$,20),
('three_images',$$Aujourd’hui en trois images$$,$$Trois instantanés pour raconter votre journée.$$,$$1.

2.

3.$$,30),
('forgotten_detail',$$Le détail que j’aurais oublié$$,$$Un petit détail banal que vous aimeriez retrouver plus tard.$$,$$Aujourd’hui, il y avait ce petit détail :$$,40),
('lingering_sentence',$$Une phrase qui reste$$,$$Une phrase entendue aujourd’hui qui vous est restée.$$,$$Quelqu’un a dit :

Pourquoi ça m’est resté :$$,50),
('future_memory',$$Le futur souvenir$$,$$La chose que vous voudriez encore vous rappeler dans dix ans.$$,$$Si je ne devais garder qu’un seul souvenir de cette journée dans dix ans, ce serait :$$,60),
('tell_this_day',$$Ce que je raconterais de cette journée$$,$$La version de cette journée que vous raconteriez plus tard.$$,$$Si quelqu’un me demandait dans quelques années « c’était comment ce jour-là ? », je raconterais :$$,70),
('someone_today',$$Quelqu’un aujourd’hui$$,$$Une personne croisée, vue ou simplement présente dans vos pensées.$$,$$Aujourd’hui, je pense à :

Ce que j’ai envie de garder de cette personne ou de ce moment :$$,80),
('unsaid',$$Ce que je n’ai pas dit$$,$$Quelque chose que vous avez pensé ou voulu dire sans le dire.$$,$$Aujourd’hui, je n’ai pas dit :$$,90),
('inner_weather',$$Météo intérieure$$,$$À quoi ressemble votre état intérieur aujourd’hui ?$$,$$Aujourd’hui, à l’intérieur, c’est plutôt :

Ce qui fait bouger le temps :

Ce dont j’aurais besoin :$$,100),
('changed_my_mind_moment',$$Le moment où j’ai changé d’avis$$,$$Un moment où quelque chose a fait bouger votre point de vue.$$,$$Au début, je pensais :

Puis :

Maintenant, je pense :$$,110),
('almost_didnt',$$Un truc que j’ai failli ne pas faire$$,$$Quelque chose que vous avez presque évité, abandonné ou raté.$$,$$J’ai presque :

Finalement :

Et :$$,120),
('unexpected_thing',$$La chose étrange ou inattendue du jour$$,$$Un détail bizarre, surprenant ou improbable.$$,$$Aujourd’hui, il y a eu ce truc :$$,130),
('three_favorite_moments',$$Mes 3 moments préférés de la journée$$,$$Les trois moments que vous avez le plus aimés aujourd’hui.$$,$$1.

2.

3.$$,140),
('three_thanks',$$Mes 3 mercis$$,$$Trois personnes, choses ou moments pour lesquels vous avez envie de dire merci.$$,$$1. Merci pour

2. Merci pour

3. Merci pour$$,150),
('unpopular_opinion',$$Unpopular opinion$$,$$Un avis que vous avez et avec lequel tout le monde ne serait probablement pas d’accord.$$,$$Je pense que :$$,160),
('unfinished_opinion',$$Mon avis pas fini$$,$$Une intuition ou une opinion que vous n’avez pas encore complètement formée.$$,$$En ce moment, j’ai l’intuition que :

Ce qui me fait penser ça :

Ce dont je ne suis pas encore sûr :$$,170),
('current_theory',$$Ma théorie du moment$$,$$Une petite théorie personnelle sur les gens, le travail, les relations, la culture ou autre chose.$$,$$Ma théorie du moment :

Pourquoi j’y crois :

Ce qui pourrait me faire changer d’avis :$$,180),
('changed_my_mind',$$J’ai changé d’avis sur…$$,$$Une idée ou un sujet sur lequel votre position a évolué.$$,$$Avant, je pensais :

Aujourd’hui, je pense :

Ce qui a changé :$$,190),
('underrated',$$Le truc sous-coté$$,$$Quelque chose qu’on ne valorise pas assez.$$,$$Je trouve que c’est largement sous-coté :

Pourquoi :$$,200),
('overrated',$$Le truc surcoté$$,$$Quelque chose qui reçoit beaucoup plus d’attention qu’il ne le mérite.$$,$$Je trouve que c’est surcoté :

Pourquoi :$$,210);
