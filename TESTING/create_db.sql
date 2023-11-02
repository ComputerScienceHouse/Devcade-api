--
-- Name: collection; Type: TABLE; Schema: devcade; Owner: devcade
--
BEGIN;
    CREATE TYPE public.UserType AS ENUM ('CSH', 'GOOGLE');
    ALTER TYPE public.UserType OWNER TO devcade;
COMMIT;

CREATE TABLE public.collection (
    collection_name character varying(100) NOT NULL,
    username character varying(32) NOT NULL
);


ALTER TABLE public.collection OWNER TO devcade;

--
-- Name: contains_game; Type: TABLE; Schema: devcade; Owner: devcade
--

CREATE TABLE public.contains_game (
    username character varying(32),
    collection_name character varying(100),
    game_id character(36)
);


ALTER TABLE public.contains_game OWNER TO devcade;

--
-- Name: game; Type: TABLE; Schema: devcade; Owner: devcade
--

CREATE TABLE public.game (
    id character(36) NOT NULL,
    author character varying(32) NOT NULL,
    upload_date date NOT NULL,
    name character varying(128) NOT NULL,
    hash character varying(255),
    description character varying(1500) NOT NULL
);


ALTER TABLE public.game OWNER TO devcade;

--
-- Name: game_tags; Type: TABLE; Schema: devcade; Owner: devcade
--

CREATE TABLE public.game_tags (
    game_id character varying(36) NOT NULL,
    tag_name character varying(32) NOT NULL
);


ALTER TABLE public.game_tags OWNER TO devcade;

--
-- Name: saves_user; Type: TABLE; Schema: devcade; Owner: devcade
--

CREATE TABLE public.saves_user (
    username character varying(32) NOT NULL,
    game_id character(36) NOT NULL
);


ALTER TABLE public.saves_user OWNER TO devcade;

--
-- Name: tags; Type: TABLE; Schema: devcade; Owner: devcade
--

CREATE TABLE public.tags (
    name character varying(32) NOT NULL,
    description text
);


ALTER TABLE public.tags OWNER TO devcade;

--
-- Name: users; Type: TABLE; Schema: devcade; Owner: devcade
--

CREATE TABLE public.users (
    id character varying(32) NOT NULL,
    user_type UserType,
    first_name character varying(32),
    last_name character varying(32),
    picture character varying(255),
    admin boolean,
    email character varying(255)
);


ALTER TABLE public.users OWNER TO devcade;

--
-- Name: collection collection_pk; Type: CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.collection
    ADD CONSTRAINT collection_pk PRIMARY KEY (collection_name, username);


--
-- Name: contains_game contains_game_pk; Type: CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.contains_game
    ADD CONSTRAINT contains_game_pk UNIQUE (username, collection_name, game_id);


--
-- Name: game game_pk; Type: CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.game
    ADD CONSTRAINT game_pk PRIMARY KEY (id);


--
-- Name: game_tags game_tags_pkey; Type: CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.game_tags
    ADD CONSTRAINT game_tags_pkey PRIMARY KEY (game_id, tag_name);


--
-- Name: saves_user saves_user_pk; Type: CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.saves_user
    ADD CONSTRAINT saves_user_pk PRIMARY KEY (username, game_id);


--
-- Name: tags tags_pkey; Type: CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.tags
    ADD CONSTRAINT tags_pkey PRIMARY KEY (name);


--
-- Name: users user_pk; Type: CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT user_pk PRIMARY KEY (id);


--
-- Name: collection collection_user_username_fk; Type: FK CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.collection
    ADD CONSTRAINT collection_user_username_fk FOREIGN KEY (username) REFERENCES public.users(id) ON UPDATE CASCADE ON DELETE CASCADE;


--
-- Name: contains_game contains_game_collection_username_collection_name_fk; Type: FK CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.contains_game
    ADD CONSTRAINT contains_game_collection_username_collection_name_fk FOREIGN KEY (username, collection_name) REFERENCES public.collection(username, collection_name) ON UPDATE CASCADE ON DELETE CASCADE;


--
-- Name: contains_game contains_game_game_game_id_fk; Type: FK CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.contains_game
    ADD CONSTRAINT contains_game_game_game_id_fk FOREIGN KEY (game_id) REFERENCES public.game(id) ON UPDATE CASCADE ON DELETE CASCADE;


--
-- Name: game_tags game_id; Type: FK CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.game_tags
    ADD CONSTRAINT game_id FOREIGN KEY (game_id) REFERENCES public.game(id) ON UPDATE CASCADE ON DELETE CASCADE;


--
-- Name: saves_user saves_user_game_game_id_fk; Type: FK CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.saves_user
    ADD CONSTRAINT saves_user_game_game_id_fk FOREIGN KEY (game_id) REFERENCES public.game(id) ON UPDATE CASCADE ON DELETE CASCADE;


--
-- Name: saves_user saves_user_user_username_fk; Type: FK CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.saves_user
    ADD CONSTRAINT saves_user_user_username_fk FOREIGN KEY (username) REFERENCES public.users(id) ON UPDATE CASCADE ON DELETE CASCADE;


--
-- Name: game_tags tag_name; Type: FK CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.game_tags
    ADD CONSTRAINT tag_name FOREIGN KEY (tag_name) REFERENCES public.tags(name) ON UPDATE CASCADE ON DELETE CASCADE;


--
-- Name: game users; Type: FK CONSTRAINT; Schema: devcade; Owner: devcade
--

ALTER TABLE ONLY public.game
    ADD CONSTRAINT users FOREIGN KEY (author) REFERENCES public.users(id) ON UPDATE CASCADE ON DELETE CASCADE;

INSERT INTO users VALUES
('skyz', 'CSH', 'Joe', 'Abbate', 'https://profiles.csh.rit.edu/image/skyz', true, 'skyz@csh.rit.edu'),
('qel', 'CSH', 'Jeremy', 'Smart', 'https://profiles.csh.rit.edu/image/qel', false, 'qel@csh.rit.edu'),
('ella', 'CSH', 'Ella', 'Soccoli', 'https://profiles.csh.rit.edu/image/ella', false, 'ella@csh.rit.edu'),
('atom', 'CSH', 'Ata', 'Noor', 'https://profiles.csh.rit.edu/image/atom', false, 'atom@csh.rit.edu'),
('joeneil', 'CSH', 'Joe', 'ONeil', 'https://profiles.csh.rit.edu/image/joeneil', true, 'joeneil@csh.rit.edu'),
('mtft', 'CSH', 'Noah', 'Emke', 'https://profiles.csh.rit.edu/image/mtft', true, 'mtft@csh.rit.edu'),
('jayh', 'CSH', 'Jay', 'Horsfall', 'https://profiles.csh.rit.edu/image/jayh', false, 'jayh@csh.rit.edu'),
('god', 'CSH', 'Brett', 'Huber', 'https://profiles.csh.rit.edu/image/god', false, 'god@csh.rit.edu'),
('rose', 'CSH', 'Johanna', 'Wichmann', 'https://profiles.csh.rit.edu/image/rose', false, 'rose@csh.rit.edu'),
('atlas', 'CSH', 'Emma', 'Schmitt', 'https://profiles.csh.rit.edu/image/atlas', false, 'atlas@csh.rit.edu'),
('wilnil', 'CSH', 'Willard', 'Nilges', 'https://profiles.csh.rit.edu/image/wilnil', false, 'wilnil@csh.rit.edu'),
('log', 'CSH', 'Logan', 'Endes', 'https://profiles.csh.rit.edu/image/log', false, 'log@csh.rit.edu'),
('tacotuesday', 'CSH', 'Conner', 'Meagher', 'https://profiles.csh.rit.edu/image/tacotuesday', false, 'tacotuesday@csh.rit.edu'),
('bigc', 'CSH', 'Connor', 'Langa', 'https://profiles.csh.rit.edu/image/bigc', false, 'bigc@csh.rit.edu'),
('evan', 'CSH', 'Evan', 'Clough', 'https://profiles.csh.rit.edu/image/evan', false, 'evan@csh.rit.edu'),
('otto', 'CSH', 'Marcus', 'Otto', 'https://profiles.csh.rit.edu/image/otto', false, 'otto@csh.rit.edu'),
('tex', 'CSH', 'Avan', 'Peltier', 'https://profiles.csh.rit.edu/image/tex', false, 'tex@csh.rit.edu'),
('PDNTSPA', 'CSH', 'Curtis', 'Heater', 'https://profiles.csh.rit.edu/image/PDNTSPA', false, 'PDNTSPA@csh.rit.edu'),
('sultanofswing', 'CSH', 'Charlie', 'Salinetti', 'https://profiles.csh.rit.edu/image/sultanofswing', false, 'sultanofswing@csh.rit.edu'),
('samc', 'CSH', 'Sam', 'Cordry', 'https://profiles.csh.rit.edu/image/samc', false, 'samc@csh.rit.edu'),
('fish', 'CSH', 'Nate', 'Aquino', 'https://profiles.csh.rit.edu/image/fish', false, 'fish@csh.rit.edu'),
('limabean', 'CSH', 'Darwin', 'Tran', 'https://profiles.csh.rit.edu/image/limabean', false, 'limabean@csh.rit.edu'),
('babysatan', 'CSH', 'Alex', 'Vasilcoiu', 'https://profiles.csh.rit.edu/image/babysatan', false, 'babysatan@csh.rit.edu'),
('theai', 'CSH', 'Ada', 'Foster', 'https://profiles.csh.rit.edu/image/theai', false, 'theai@csh.rit.edu'),
('sihang', 'CSH', 'Sihang', 'Hu', 'https://profiles.csh.rit.edu/image/sihang', false, 'sihang@csh.rit.edu'),
('nintendods', 'CSH', 'Dani', 'Saba', 'https://profiles.csh.rit.edu/image/nintendods', false, 'nintendods@csh.rit.edu');

INSERT INTO tags VALUES
('TestTag1', 'TestTag1 Description'),
('TestTag2', 'TestTag2 Description'),
('TestTag3', 'TestTag3 Description'),
('TestTag4', 'TestTag4 Description'),
('TestTag5', 'TestTag5 Description'),
('TestTag6', 'TestTag6 Description'),
('TestTag7', 'TestTag7 Description'),
('TestTag8', 'TestTag8 Description'),
('TestTag9', 'TestTag9 Description'),
('TestTag10', 'TestTag10 Description'),
('TestTag11', 'TestTag11 Description'),
('TestTag12', 'TestTag12 Description'),
('TestTag13', 'TestTag13 Description'),
('TestTag14', 'TestTag14 Description'),
('TestTag15', 'TestTag15 Description'),
('TestTag16', 'TestTag16 Description'),
('TestTag17', 'TestTag17 Description'),
('TestTag18', 'TestTag18 Description'),
('TestTag19', 'TestTag19 Description'),
('TestTag20', 'TestTag20 Description');


INSERT INTO game VALUES
('AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA', 'skyz', '2023-03-23', 'TestGameA', '5ec8f244899431af8effad9e7ec9b2543226c78f', 'TestGameA Description'),
('BBBBBBBB-BBBB-BBBB-BBBB-BBBBBBBBBBBB', 'qel', '2023-03-23', 'TestGameB', '7b91c68006227a64c3cabcb6aa67f3e3e792b11b', 'TestGameB Description'),
('CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC', 'ella', '2023-03-23', 'TestGameC', '0d641140903c21d47a007b0136e7c2a7295a254a', 'TestGameC Description'),
('DDDDDDDD-DDDD-DDDD-DDDD-DDDDDDDDDDDD', 'atom', '2023-03-23', 'TestGameD', '04d6c7defa5dd48067cb44a473ac8eeb17f529f5', 'TestGameD Description'),
('EEEEEEEE-EEEE-EEEE-EEEE-EEEEEEEEEEEE', 'joeneil', '2023-03-23', 'TestGameE', '5d4ac1284877c9262df5808b8ab0e922863f9464', 'TestGameE Description'),
-- ('FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF', 'mtft', '2023-03-23', 'TestGameF', 'cb838a5177364dacaaeff3724d27202729ad4427', 'TestGameF Description'),
('GGGGGGGG-GGGG-GGGG-GGGG-GGGGGGGGGGGG', 'skyz', '2023-03-23', 'TestGameG', '3bb390de22dbc674b993e33536bd53c6851a7290', 'TestGameG Description');
-- ('HHHHHHHH-HHHH-HHHH-HHHH-HHHHHHHHHHHH', 'skyz', '2023-03-23', 'TestGameH', '579e03f4fdad803a53602808ce2cfaead7c69344', 'TestGameH Description'),
-- ('IIIIIIII-IIII-IIII-IIII-IIIIIIIIIIII', 'skyz', '2023-03-23', 'TestGameI', '6f6e1f0733bc60463d32436d2c115382ec6a801f', 'TestGameI Description'),
-- ('JJJJJJJJ-JJJJ-JJJJ-JJJJ-JJJJJJJJJJJJ', 'skyz', '2023-03-23', 'TestGameJ', 'a5e8a81726700bc1b408cb60366f232ada0e726b', 'TestGameJ Description'),
-- ('KKKKKKKK-KKKK-KKKK-KKKK-KKKKKKKKKKKK', 'skyz', '2023-03-23', 'TestGameK', '8b4290df8ecdd83dbd215fe745499c0f5e492e28', 'TestGameK Description'),
-- ('LLLLLLLL-LLLL-LLLL-LLLL-LLLLLLLLLLLL', 'skyz', '2023-03-23', 'TestGameL', 'f942b92d813a16ab1ef322e8ad7b1a15d42390b5', 'TestGameL Description'),
-- ('MMMMMMMM-MMMM-MMMM-MMMM-MMMMMMMMMMMM', 'skyz', '2023-03-23', 'TestGameM', '0d54118dcfd7105ac57008a835998f5a08488368', 'TestGameM Description'),
-- ('NNNNNNNN-NNNN-NNNN-NNNN-NNNNNNNNNNNN', 'skyz', '2023-03-23', 'TestGameN', 'e5cc1088ea7326b246364e3fa40a97020f2662b9', 'TestGameN Description'),
-- ('OOOOOOOO-OOOO-OOOO-OOOO-OOOOOOOOOOOO', 'skyz', '2023-03-23', 'TestGameO', '7f5a092a1adb4d4a62fce40d2cc24b6510b0e30a', 'TestGameO Description'),
-- ('PPPPPPPP-PPPP-PPPP-PPPP-PPPPPPPPPPPP', 'skyz', '2023-03-23', 'TestGameP', '8d2d4871bb22b6cc79d3e10db94cd565b3fb36f4', 'TestGameP Description'),
-- ('QQQQQQQQ-QQQQ-QQQQ-QQQQ-QQQQQQQQQQQQ', 'skyz', '2023-03-23', 'TestGameQ', '555d9338b6814340a9370d9a1d6eda87f868de08', 'TestGameQ Description'),
-- ('RRRRRRRR-RRRR-RRRR-RRRR-RRRRRRRRRRRR', 'skyz', '2023-03-23', 'TestGameR', 'a3a7439557ab4f12ce05161f44f59d503eb64811', 'TestGameR Description'),
-- ('SSSSSSSS-SSSS-SSSS-SSSS-SSSSSSSSSSSS', 'skyz', '2023-03-23', 'TestGameS', '232e58b3c37ffba7a23cfcfb566d2a627c631b5b', 'TestGameS Description'),
-- ('TTTTTTTT-TTTT-TTTT-TTTT-TTTTTTTTTTTT', 'skyz', '2023-03-23', 'TestGameT', '9cf1500fe596e2b495e18dd8c34b6fce8ad18db5', 'TestGameT Description'),
-- ('UUUUUUUU-UUUU-UUUU-UUUU-UUUUUUUUUUUU', 'skyz', '2023-03-23', 'TestGameU', '9546a29a58b26d7f49c1325171c5fe8e667fd8fd', 'TestGameU Description'),
-- ('VVVVVVVV-VVVV-VVVV-VVVV-VVVVVVVVVVVV', 'skyz', '2023-03-23', 'TestGameV', 'bb0f9325aadfa25e1bc0b6390c9fd6fea6c5487f', 'TestGameV Description'),
-- ('WWWWWWWW-WWWW-WWWW-WWWW-WWWWWWWWWWWW', 'skyz', '2023-03-23', 'TestGameW', '06095b71ee690c46fbb2ad3090d1e00f4d13cd67', 'TestGameW Description'),
-- ('XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX', 'skyz', '2023-03-23', 'TestGameX', 'ee5a7cc7c8c9c3f7ae8110dd6c2dcb4153152643', 'TestGameX Description'),
-- ('YYYYYYYY-YYYY-YYYY-YYYY-YYYYYYYYYYYY', 'skyz', '2023-03-23', 'TestGameY', 'e500d0c0816f7c0b2f12890a711d7378725ca9a0', 'TestGameY Description'),
-- ('ZZZZZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZZZZZZZZZ', 'skyz', '2023-03-23', 'TestGameZ', 'e4553dd5e307c7ce3aded8ca857c46e18a7935e9', 'TestGameZ Description');

INSERT INTO game_tags VALUES
('AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA', 'TestTag1'),
('CCCCCCCC-CCCC-CCCC-CCCC-CCCCCCCCCCCC', 'TestTag4');
